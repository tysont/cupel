// ABOUTME: `gen.differential` (ADR-094): entry points whose answers on generated inputs differ from the run's
// ABOUTME: oracle, read from DivergenceResults and located through Symbols and NormTree, one finding per entry.
use std::collections::{BTreeMap, HashMap};

use cupel_core::config::OracleKind;
use cupel_core::syntax::{NormTree, SymbolDef, SymbolKey, Symbols};
use cupel_core::{
    ActionKind, Answer, Check, CheckCtx, CheckId, CheckResult, CheckStatus, ComparedOracle,
    CostClass, Divergence, DivergenceResults, EntryComparison, Event, EventSink, FactKind, Facts,
    Finding, Manifest, Plan, Result, Scope, Score, Severity, SkipReason, Skipped, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::finding::SymbolFinding;
use crate::manifest::{kind, metric};
use crate::metrics::{hashes_by_symbol, sum};
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "gen.differential";
const KIND: &str = "diff.divergence";

/// The check takes no settings; entry points and the case budget are the engine's
/// (`[lang.<id>] differential_entries`, `--cases`).
const LIMITS: Limits = Limits::new(&[]);

/// Where a skip for a `spec` oracle points the reader.
const SEE: &str = "cupel describe gen.differential";

/// Evidence of `diff.divergence`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Diverged {
    /// The entry point as configured: `<file>:<function>`.
    entry: String,
    /// The oracle compared against.
    oracle: Against,
    /// The minimized arguments.
    input: Vec<Value>,
    /// The arguments as first generated.
    found: Vec<Value>,
    /// The index of `found` among the entry's inputs.
    case: u64,
    /// Minimization steps from `found` to `input`.
    shrinks: u32,
    /// What the working tree answered on `input`.
    implementation: Said,
    /// What the oracle answered on `input`.
    reference: Said,
}

/// The oracle a divergence is against; inlined, as evidence schemas have no `$defs` (ADR-043).
#[derive(Serialize, JsonSchema)]
#[schemars(inline)]
#[serde(deny_unknown_fields)]
struct Against {
    name: String,
    /// `ref` or `command`.
    kind: String,
    /// The commit a `ref` oracle resolved to.
    commit: Option<String>,
}

/// How one call ended: exactly one key, `returned` (the value's rendering), `raised` (the
/// error), or `crashed` (why there was no answer).
#[derive(Serialize, JsonSchema)]
#[schemars(inline)]
#[serde(deny_unknown_fields)]
struct Said {
    #[serde(skip_serializing_if = "Option::is_none")]
    returned: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    raised: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    crashed: Option<String>,
}

impl From<&Answer> for Said {
    fn from(answer: &Answer) -> Self {
        let text = Some(answer.text().to_owned());
        let mut said = Self {
            returned: None,
            raised: None,
            crashed: None,
        };
        match answer {
            Answer::Returned(_) => said.returned = text,
            Answer::Raised(_) => said.raised = text,
            Answer::Crashed(_) => said.crashed = text,
        }
        said
    }
}

pub struct GenDifferential {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(GenDifferential::new())));

impl GenDifferential {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(4).expect("tier 4 exists"),
                cost: CostClass::Deep,
                needs: vec![
                    FactKind::DivergenceResults,
                    FactKind::Symbols,
                    FactKind::NormTree,
                ],
                wants: Vec::new(),
                findings: vec![kind::<Diverged>(
                    KIND,
                    ActionKind::FixBug,
                    "An entry point answered a generated input differently from the oracle; \
                     the input is minimized and both answers are in the evidence.",
                )],
                metrics: vec![
                    metric(
                        "divergences",
                        "Entry points whose answers differ from the oracle's.",
                    ),
                    metric(
                        "cases",
                        "Inputs both the implementation and the oracle answered.",
                    ),
                ],
                settings: LIMITS.specs(),
                stability: Stability::Experimental,
                summary: "Calls configured entry points on generated inputs in the working tree \
                          and against the run's oracle (a ref's commit or a command); reports the \
                          minimized inputs on which they answer differently."
                    .to_owned(),
            },
        }
    }
}

impl Check for GenDifferential {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// One target per file in scope; the entries and inputs are the engine's to know.
    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        LIMITS.plan(
            &self.manifest,
            ctx,
            outcome::plan_files(&self.manifest, scope),
        )
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        LIMITS.read(&self.manifest.id, &ctx.config)?;
        let results = facts.get::<DivergenceResults>()?;
        let Some(oracle) = &results.oracle else {
            return Ok(self.nothing_compared(vec!["the run has no oracle".to_owned()]));
        };
        if oracle.kind == OracleKind::Spec {
            return Ok(self.skipped());
        }
        let symbols: HashMap<SymbolKey, &SymbolDef> = facts
            .get::<Symbols>()?
            .symbols
            .iter()
            .map(|symbol| (symbol.key(), symbol))
            .collect();
        let trees = facts.get::<NormTree>()?;
        let hashes = hashes_by_symbol(trees);
        let mut notes = Vec::new();
        let (mut entries, mut divergent, mut cases) = (0_u64, 0_u64, 0_u64);
        for entry in &results.entries {
            if let Some(reason) = &entry.skipped {
                notes.push(format!("{} not compared: {reason}", entry.entry));
                continue;
            }
            entries += 1;
            cases += entry.compared;
            if let Some(finding) = self.finding(results, oracle, entry, &symbols, &hashes) {
                divergent += 1;
                sink.emit(Event::Finding(Box::new(finding)));
            }
        }
        if entries == 0 {
            return Ok(self.nothing_compared(notes));
        }
        let mut result = self.compared(divergent, entries, cases);
        result.notes = notes;
        Ok(result)
    }

    /// Its results name the run's oracle and are capped at tier 3 against an unverified one
    /// (ADR-091).
    fn uses_oracle(&self) -> bool {
        true
    }

    /// Divergences and inputs add up across languages.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }

    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }
}

impl GenDifferential {
    /// The finding for `entry`'s divergence, if it diverged; none for a function with no
    /// symbol or structural hash, which cannot be identified.
    fn finding(
        &self,
        results: &DivergenceResults,
        oracle: &ComparedOracle,
        entry: &EntryComparison,
        symbols: &HashMap<SymbolKey, &SymbolDef>,
        hashes: &HashMap<&SymbolKey, &cupel_core::SymbolHash>,
    ) -> Option<Finding> {
        let divergence = entry.divergence.as_ref()?;
        let key = entry.symbol.as_ref()?;
        let (symbol, hash) = (symbols.get(key)?, hashes.get(key)?);
        let call = format!("`{}({})`", symbol.name, arguments(&divergence.input));
        let evidence = Diverged {
            entry: entry.entry.clone(),
            oracle: Against {
                name: oracle.name.clone(),
                kind: oracle.kind.as_str().to_owned(),
                commit: oracle.commit.clone(),
            },
            input: divergence.input.clone(),
            found: divergence.found.clone(),
            case: divergence.case,
            shrinks: divergence.shrinks,
            implementation: Said::from(&divergence.implementation),
            reference: Said::from(&divergence.oracle),
        };
        let mut finding = SymbolFinding::new(&self.manifest, KIND, symbol, hash, evidence)
            .described(
                title(&call, &oracle.name, divergence),
                format!(
                    "Decide which answer is right for {call}: fix the implementation, or, if the \
                     change is intended, the oracle."
                ),
            )
            .at(Severity::Fail)
            .finding();
        finding.repro = format!(
            "{} --oracle {} --seed {} --cases {}",
            finding.repro, oracle.name, results.seed, results.cases
        );
        Some(finding)
    }

    fn compared(&self, divergent: u64, entries: u64, cases: u64) -> CheckResult {
        #[expect(
            clippy::cast_precision_loss,
            reason = "entry and input counts stay far below 2^52, where f64 is exact"
        )]
        let metrics = BTreeMap::from([
            ("divergences".to_owned(), divergent as f64),
            ("cases".to_owned(), cases as f64),
        ]);
        #[expect(
            clippy::cast_precision_loss,
            reason = "entry counts stay far below 2^52, where f64 is exact"
        )]
        let score = Score {
            value: divergent as f64,
            unit: "divergent".to_owned(),
            n: entries,
        };
        outcome::result(
            &self.manifest,
            outcome::status(divergent > 0),
            Some(score),
            metrics,
            divergent,
        )
    }

    /// A pass with no score and no metrics, so a gate rule on them is skipped rather than
    /// passing on nothing (ADR-094 §1).
    fn nothing_compared(&self, notes: Vec<String>) -> CheckResult {
        let mut result =
            outcome::result(&self.manifest, CheckStatus::Pass, None, BTreeMap::new(), 0);
        result.notes = notes;
        result
    }

    /// The check's own skip when the run's oracle is a specification (ADR-094 §3).
    fn skipped(&self) -> CheckResult {
        let mut result = outcome::result(
            &self.manifest,
            CheckStatus::Skipped,
            None,
            BTreeMap::new(),
            0,
        );
        result.skipped = Some(Skipped {
            reason: SkipReason::Unconfigured,
            fact: None,
            lang: None,
            tool: None,
            see: Some(SEE.to_owned()),
            est_ms: None,
        });
        result
    }
}

/// The arguments as a call writes them: each JSON value, comma-separated.
fn arguments(input: &[Value]) -> String {
    input
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// How the two sides ended on the input, as the finding's title says it.
fn title(call: &str, oracle: &str, divergence: &Divergence) -> String {
    format!(
        "{call} {}, but oracle `{oracle}` {}",
        ended(&divergence.implementation),
        ended(&divergence.oracle)
    )
}

fn ended(answer: &Answer) -> String {
    match answer {
        Answer::Returned(text) => format!("returned `{text}`"),
        Answer::Raised(text) => format!("raised `{text}`"),
        Answer::Crashed(text) => format!("crashed ({text})"),
    }
}
