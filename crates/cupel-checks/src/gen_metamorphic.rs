// ABOUTME: `gen.metamorphic`: metamorphic relations that do not hold (ADR-098), read from RelationResults: one failing
// ABOUTME: finding per violated relation, with the minimized input, the follow-up input, and both outputs in evidence.
use std::collections::BTreeMap;

use cupel_core::{
    Action, ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Evaluated, Event,
    EventSink, Evidence, FactKind, Facts, Finding, FindingId, Manifest, Plan, RelationResult,
    RelationResults, Result, Scope, Score, Severity, Stability, Tier, Violation,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::manifest::{evidence, kind, metric};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "gen.metamorphic";

const KIND: &str = "metamorphic.violated";

/// The check takes no settings; relations and their case budgets are the engine's
/// (`[lang.<id>.relations]`).
const LIMITS: Limits = Limits::new(&[]);

/// How one evaluation ended: exactly one of its keys. Inlined in the evidence schema, which has
/// no `$defs` (ADR-043).
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(inline)]
struct Outcome {
    /// What it returned, when JSON can hold it.
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Value>,
    /// What it raised: the exception's type and message.
    #[serde(skip_serializing_if = "Option::is_none")]
    raised: Option<String>,
    /// What it returned, printed by the language, when JSON cannot hold it.
    #[serde(skip_serializing_if = "Option::is_none")]
    repr: Option<String>,
}

impl From<&Evaluated> for Outcome {
    fn from(evaluated: &Evaluated) -> Self {
        let none = Self {
            value: None,
            raised: None,
            repr: None,
        };
        match evaluated {
            Evaluated::Value(value) => Self {
                value: Some(value.clone()),
                ..none
            },
            Evaluated::Raised(raised) => Self {
                raised: Some(raised.clone()),
                ..none
            },
            Evaluated::Repr(repr) => Self {
                repr: Some(repr.clone()),
                ..none
            },
        }
    }
}

/// Evidence of `metamorphic.violated`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Violated {
    /// The relation's name, as configured.
    relation: String,
    /// The function under test, as configured.
    function: String,
    /// The type of the generated inputs.
    input_type: String,
    /// The minimized input the relation does not hold for.
    input: Value,
    /// The follow-up input: the transform applied to `input`.
    transformed: Outcome,
    /// The function on `input`.
    source_output: Outcome,
    /// The function on the follow-up input; null when the transform raised.
    followup_output: Option<Outcome>,
    /// What the relation raised on the two outputs; null when it returned false or was not
    /// called.
    relation_raised: Option<String>,
    /// The generated input before minimization.
    original_input: Value,
    /// The index of the generated case that first violated the relation.
    case: u64,
    /// Minimization steps from `original_input` to `input`.
    shrinks: u32,
    /// The run's seed, which regenerates the same inputs.
    seed: u64,
}

pub struct GenMetamorphic {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(GenMetamorphic::new())));

impl GenMetamorphic {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(4).expect("tier 4 exists"),
                cost: CostClass::Deep,
                needs: vec![FactKind::RelationResults],
                wants: Vec::new(),
                findings: vec![kind::<Violated>(
                    KIND,
                    ActionKind::FixBug,
                    "A configured metamorphic relation does not hold: the function's outputs on a \
                     generated input and on its transform do not relate as declared.",
                )],
                metrics: ["violated", "relations"]
                    .into_iter()
                    .zip([
                        "Relations some generated input violated.",
                        "Relations evaluated.",
                    ])
                    .map(|(name, summary)| metric(name, summary))
                    .collect(),
                settings: LIMITS.specs(),
                stability: Stability::Experimental,
                summary: "Evaluates the metamorphic relations declared in configuration on \
                          generated inputs; reports each relation that does not hold, with a \
                          minimized input."
                    .to_owned(),
            },
        }
    }
}

impl Check for GenMetamorphic {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// One target per file in scope; how many relations there are is the engine's to know.
    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        LIMITS.plan_files(&self.manifest, ctx, scope)
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        LIMITS.read(&self.manifest.id, &ctx.config)?;
        let results = facts.get::<RelationResults>()?;
        let in_scope: Vec<&RelationResult> = results
            .relations
            .iter()
            .filter(|result| in_scope(&ctx.scope, result))
            .collect();
        let findings: Vec<Finding> = in_scope
            .iter()
            .filter_map(|result| {
                let violation = result.violation.as_ref()?;
                Some(self.finding(result, violation, results.seed))
            })
            .collect();
        let (violated, evaluated) = (findings.len() as u64, in_scope.len() as u64);
        for finding in findings {
            sink.emit(Event::Finding(Box::new(finding)));
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "relation counts stay far below 2^52, where f64 is exact"
        )]
        let metrics = BTreeMap::from([
            ("violated".to_owned(), violated as f64),
            ("relations".to_owned(), evaluated as f64),
        ]);
        #[expect(
            clippy::cast_precision_loss,
            reason = "relation counts stay far below 2^52, where f64 is exact"
        )]
        let score = Score {
            value: violated as f64,
            unit: "violated".to_owned(),
            n: evaluated,
        };
        Ok(outcome::result(
            &self.manifest,
            outcome::status(violated > 0),
            Some(score),
            metrics,
            violated,
        ))
    }

    /// Violated relations add up across languages.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }

    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }
}

/// Whether `result`'s function is defined in a file in scope; a relation whose harness could
/// not locate its function is always in scope.
fn in_scope(scope: &Scope, result: &RelationResult) -> bool {
    scope.files.is_empty()
        || result
            .location
            .as_ref()
            .is_none_or(|location| scope.files.contains(&location.file))
}

impl GenMetamorphic {
    fn finding(&self, result: &RelationResult, violation: &Violation, seed: u64) -> Finding {
        let subject = format!("{} {}", result.name, result.function);
        let id = FindingId::for_subject(&self.manifest.id, KIND, &subject);
        let fields = match evidence(&Violated {
            relation: result.name.clone(),
            function: result.function.clone(),
            input_type: result.input.clone(),
            input: violation.input.clone(),
            transformed: Outcome::from(&violation.transformed),
            source_output: Outcome::from(&violation.source),
            followup_output: violation.followup.as_ref().map(Outcome::from),
            relation_raised: violation.relation_raised.clone(),
            original_input: violation.original.clone(),
            case: violation.case,
            shrinks: violation.shrinks,
            seed,
        }) {
            Value::Object(fields) => fields,
            _ => serde_json::Map::new(),
        };
        let (name, function) = (&result.name, &result.function);
        Finding {
            suppress: format!("cupel suppress {id} --reason '…'"),
            repro: format!("cupel check --only {} --seed {seed}", self.manifest.id),
            id,
            check: self.manifest.id.clone(),
            kind: KIND.to_owned(),
            severity: Severity::Fail,
            confidence: 1.0,
            tier: self.manifest.tier,
            symbol: None,
            location: result.location.clone(),
            title: format!("`{function}` breaks the relation `{name}`"),
            detail: None,
            evidence: Evidence(fields),
            action: Action {
                kind: ActionKind::FixBug,
                priority: 1,
                summary: format!(
                    "Fix `{function}` so that `{name}` holds for the input {}; the evidence \
                     has the follow-up input and both outputs.",
                    violation.input
                ),
                hints: serde_json::Map::new(),
            },
            new_since_baseline: false,
            suppressed: false,
        }
    }
}
