// ABOUTME: `tests.mutation`: scores MutationOutcomes, whoever produced them (ADR-080): surviving mutants as failures at
// ABOUTME: their site, timeouts and uncovered sites as notes, ADR-004's equivalents, ADR-082's fallbacks, killed ratio.
use std::collections::{BTreeMap, HashMap};

use cupel_core::syntax::{SymbolDef, SymbolKey, Symbols, TestConventions};
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, FactKind,
    Facts, Finding, Manifest, Measurement, MetricPer, MetricSpec, Mutant, MutantOutcome,
    MutationOutcomes, MutationStrategy, Plan, Result, Scope, Score, Severity, SiteHash, Stability,
    SymbolHash, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::finding::{SymbolFinding, with_nearest_test};
use crate::manifest::{kind, metric, ranked};
use crate::metrics::{scored, sum, symbol_target, weighted_mean};
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "tests.mutation";

/// The check takes no settings; how mutants run is the engine's (`[lang.<id>] mutation`).
const SETTINGS: Limits = Limits::new(&[]);

/// The confidence of a possibly equivalent survivor (ADR-004).
const EQUIVALENT_CONFIDENCE: f64 = 0.3;

/// What was mutated; inlined in each evidence schema, which has no `$defs` (ADR-043).
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(inline)]
struct Mutation {
    /// The source text replaced.
    from: String,
    /// What replaced it.
    to: String,
    /// The mutation operator, such as `relational`.
    operator: String,
}

/// Evidence of `mutant.survived`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Survived {
    mutation: Mutation,
    /// The tests that ran against the mutant and passed.
    covering_tests: Vec<String>,
    tests_run: u32,
}

/// Evidence of `mutant.timeout` and `mutant.uncovered`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Site {
    mutation: Mutation,
}

/// Evidence of `mutant.equivalent?`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Equivalent {
    mutation: Mutation,
    /// Why the mutant may behave exactly as the original.
    reason: String,
}

pub struct TestsMutation {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(TestsMutation::new())));

impl TestsMutation {
    pub fn new() -> Self {
        Self {
            manifest: manifest(),
        }
    }
}

/// The manifest: needs, wants, finding kinds with their evidence, and metrics (ADR-080).
fn manifest() -> Manifest {
    Manifest {
        id: CheckId::from(ID),
        version: 1,
        tier: Tier::try_from(3).expect("tier 3 exists"),
        cost: CostClass::Standard,
        needs: vec![FactKind::Symbols, FactKind::MutationOutcomes],
        wants: vec![FactKind::TestConventions],
        findings: vec![
            kind::<Survived>(
                "mutant.survived",
                ActionKind::WriteTest,
                "Every test that ran against the mutant passed: no test tells the mutated \
                 code from the original.",
            ),
            kind::<Site>(
                "mutant.timeout",
                ActionKind::Review,
                "The tests ran past their timeout against the mutant, which usually means it \
                 loops.",
            ),
            kind::<Site>(
                "mutant.uncovered",
                ActionKind::WriteTest,
                "No test runs the site, so its mutant was not run.",
            ),
            kind::<Equivalent>(
                "mutant.equivalent?",
                ActionKind::Review,
                "The mutant survived and may behave exactly as the original (ADR-004).",
            ),
        ],
        metrics: metrics(),
        stability: Stability::Experimental,
        summary: "Mutates the code in scope and runs the tests that cover each mutant; a \
                  mutant every test passes against survived. Score: killed / (killed + \
                  survived). Uncovered sites are not run; outcomes of unchanged code and \
                  tests are reused from earlier runs."
            .to_owned(),
    }
}

/// The metrics this check reports: gate keys, then what `survey` ranks.
fn metrics() -> Vec<MetricSpec> {
    let mut metrics: Vec<MetricSpec> = [
        (
            "score",
            "Killed / (killed + survived), the score; 1 when nothing was scored.",
        ),
        ("killed", "Mutants a test failed against."),
        (
            "survived",
            "Mutants every test passed against, not counting possibly equivalent ones.",
        ),
        ("timeout", "Mutants whose tests ran past their timeout."),
        ("uncovered", "Mutants no test runs, which were not run."),
        ("unviable", "Mutants that did not build or load."),
        (
            "equivalent",
            "Survivors that may behave exactly as the original (ADR-004).",
        ),
        ("executed", "Mutants run this time."),
        ("reused", "Mutants whose outcome came from an earlier run."),
        (
            "schemata",
            "Mutants run this time behind one schemata build of their file (ADR-082).",
        ),
        (
            "fallback",
            "Mutants run this time as file copies in a language with schemata; the notes say why.",
        ),
    ]
    .into_iter()
    .map(|(name, summary)| metric(name, summary))
    .collect();
    metrics.push(ranked(
        "survivors",
        MetricPer::Symbol,
        "Mutants of the function that survived.",
    ));
    metrics
}

impl Check for TestsMutation {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        SETTINGS.plan(
            &self.manifest,
            ctx,
            outcome::plan_symbols(&self.manifest, scope),
        )
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        SETTINGS.read(&self.manifest.id, &ctx.config)?;
        let outcomes = facts.get::<MutationOutcomes>()?;
        let symbols: HashMap<SymbolKey, &SymbolDef> = facts
            .get::<Symbols>()?
            .symbols
            .iter()
            .map(|symbol| (symbol.key(), symbol))
            .collect();
        let conventions = facts.get::<TestConventions>().ok();
        let mut tally = Tally::default();
        let mut survivors: BTreeMap<&SymbolKey, (&SymbolDef, Option<&SymbolHash>, u32)> =
            BTreeMap::new();
        for mutant in outcomes
            .mutants
            .iter()
            .filter(|mutant| ctx.scope.includes(&mutant.symbol))
        {
            let Some(symbol) = symbols.get(&mutant.symbol) else {
                continue;
            };
            tally.count(mutant);
            let entry =
                survivors
                    .entry(&mutant.symbol)
                    .or_insert((symbol, mutant.symbol_hash.as_ref(), 0));
            entry.2 += u32::from(surviving(mutant));
            if let Some(finding) = self.finding(mutant, symbol, conventions) {
                tally.findings += 1;
                sink.emit(Event::Finding(Box::new(finding)));
            }
        }
        for (symbol, hash, survived) in survivors.into_values() {
            sink.measure(Measurement {
                metric: "survivors".to_owned(),
                value: f64::from(survived),
                target: symbol_target(symbol, hash),
            });
        }
        Ok(tally.result(&self.manifest))
    }

    /// The killed ratio over every language's scored mutants together.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::weighted(scores)
    }

    /// The score weighted by the mutants each language scored; counts add up.
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        if name == "score" {
            weighted_mean(name, results, scored)
        } else {
            sum(name, results)
        }
    }
}

/// Whether `mutant` survived and counts against the score: not possibly equivalent.
fn surviving(mutant: &Mutant) -> bool {
    mutant.outcome == MutantOutcome::Survived && mutant.equivalent.is_none()
}

/// What the mutants in scope added up to.
#[derive(Default)]
struct Tally {
    by_outcome: BTreeMap<&'static str, u64>,
    equivalent: u64,
    executed: u64,
    reused: u64,
    findings: u64,
    schemata: u64,
    /// Mutants run as file copies where schemata could not hold them, by why.
    fallbacks: BTreeMap<String, u64>,
}

impl Tally {
    fn count(&mut self, mutant: &Mutant) {
        if mutant.outcome == MutantOutcome::Survived && mutant.equivalent.is_some() {
            self.equivalent += 1;
        } else {
            *self.by_outcome.entry(mutant.outcome.as_str()).or_default() += 1;
        }
        if mutant.reused {
            self.reused += 1;
        } else if mutant.outcome != MutantOutcome::Uncovered {
            self.executed += 1;
            self.schemata += u64::from(mutant.strategy == Some(MutationStrategy::Schemata));
            if let Some(reason) = &mutant.fallback {
                *self.fallbacks.entry(reason.clone()).or_default() += 1;
            }
        }
    }

    /// One note per reason mutants ran as file copies instead of schemata.
    fn notes(&self) -> Vec<String> {
        self.fallbacks
            .iter()
            .map(|(reason, n)| match n {
                1 => format!("1 mutant ran as a file copy: {reason}"),
                n => format!("{n} mutants ran as file copies: {reason}"),
            })
            .collect()
    }

    fn of(&self, outcome: MutantOutcome) -> u64 {
        self.by_outcome.get(outcome.as_str()).copied().unwrap_or(0)
    }

    /// The language's result: `fail` with a survivor, scored by the killed ratio.
    fn result(&self, manifest: &Manifest) -> CheckResult {
        let (killed, survived) = (
            self.of(MutantOutcome::Killed),
            self.of(MutantOutcome::Survived),
        );
        let scored = killed + survived;
        let score = if scored == 0 {
            1.0
        } else {
            count(killed) / count(scored)
        };
        let mut metrics: BTreeMap<String, f64> = [
            MutantOutcome::Killed,
            MutantOutcome::Survived,
            MutantOutcome::Timeout,
            MutantOutcome::Uncovered,
            MutantOutcome::Unviable,
        ]
        .into_iter()
        .map(|outcome| (outcome.as_str().to_owned(), count(self.of(outcome))))
        .collect();
        metrics.insert("score".to_owned(), score);
        metrics.insert("equivalent".to_owned(), count(self.equivalent));
        metrics.insert("executed".to_owned(), count(self.executed));
        metrics.insert("reused".to_owned(), count(self.reused));
        metrics.insert("schemata".to_owned(), count(self.schemata));
        metrics.insert("fallback".to_owned(), count(self.fallbacks.values().sum()));
        let mut result = outcome::result(
            manifest,
            outcome::status(survived > 0),
            Some(Score {
                value: score,
                unit: "killed_ratio".to_owned(),
                n: scored,
            }),
            metrics,
            self.findings,
        );
        result.notes = self.notes();
        result
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "mutant counts stay far below 2^52, where f64 is exact"
)]
fn count(value: u64) -> f64 {
    value as f64
}

impl TestsMutation {
    /// The finding `mutant`'s outcome calls for, if any; none for a symbol with no structural
    /// hash, which cannot be identified.
    fn finding(
        &self,
        mutant: &Mutant,
        symbol: &SymbolDef,
        conventions: Option<&TestConventions>,
    ) -> Option<Finding> {
        let hash = mutant.symbol_hash.as_ref()?;
        let mutation = Mutation {
            from: mutant.from.clone(),
            to: mutant.to.clone(),
            operator: operator(mutant),
        };
        if surviving(mutant) {
            return Some(self.survived(mutant, symbol, hash, mutation, conventions));
        }
        let note = note(mutant, &symbol.file, mutation)?;
        let mut finding =
            SymbolFinding::new(&self.manifest, note.kind, symbol, hash, note.evidence)
                .described(note.title, note.summary)
                .at(Severity::Info)
                .at_site(&site_of(mutant), &mutant.span);
        if note.kind == "mutant.equivalent?" {
            finding.confidence = EQUIVALENT_CONFIDENCE;
        }
        Some(with_nearest_test(finding, symbol, conventions))
    }

    /// `mutant.survived` about `mutant`, with the tests that missed it.
    fn survived(
        &self,
        mutant: &Mutant,
        symbol: &SymbolDef,
        hash: &SymbolHash,
        mutation: Mutation,
        conventions: Option<&TestConventions>,
    ) -> Finding {
        let at = format!("{}:{}", symbol.file, mutant.span.line);
        let evidence = Survived {
            mutation,
            covering_tests: mutant.tests.clone(),
            tests_run: u32::try_from(mutant.tests.len()).unwrap_or(u32::MAX),
        };
        let mut finding =
            SymbolFinding::new(&self.manifest, "mutant.survived", symbol, hash, evidence)
                .described(
                    format!(
                        "Mutant survived: `{}` → `{}` at {at}",
                        mutant.from, mutant.to
                    ),
                    format!(
                        "Add a test that fails when `{}` at {at} becomes `{}`.",
                        mutant.from, mutant.to
                    ),
                )
                .at(Severity::Fail)
                .at_site(&site_of(mutant), &mutant.span);
        finding.confidence = 0.95;
        finding.action.hints.insert(
            "covering_tests".to_owned(),
            Value::Array(mutant.tests.iter().cloned().map(Value::String).collect()),
        );
        with_nearest_test(finding, symbol, conventions)
    }
}

/// An informational finding about one mutant: its kind, evidence, title, and action summary.
struct Note {
    kind: &'static str,
    evidence: Value,
    title: String,
    summary: String,
}

/// The note `mutant`, in `file`, calls for when it is not a plain survivor: possibly
/// equivalent, timed out, or uncovered; none for one killed or unviable.
fn note(mutant: &Mutant, file: &str, mutation: Mutation) -> Option<Note> {
    let at = format!("{file}:{}", mutant.span.line);
    let change = format!("`{}` → `{}`", mutant.from, mutant.to);
    let (kind, evidence, title, summary) = match (mutant.outcome, &mutant.equivalent) {
        (MutantOutcome::Survived, Some(reason)) => (
            "mutant.equivalent?",
            serde_json::to_value(Equivalent {
                mutation,
                reason: reason.clone(),
            }),
            format!("Mutant may be equivalent: {change} at {at}"),
            format!(
                "Review whether any input tells {change} apart at {at}; if none does, \
                 suppress this finding."
            ),
        ),
        (MutantOutcome::Timeout, _) => (
            "mutant.timeout",
            serde_json::to_value(Site { mutation }),
            format!("Mutant timed out: {change} at {at}"),
            format!("Review {change} at {at}: the tests ran past their timeout against it."),
        ),
        (MutantOutcome::Uncovered, _) => (
            "mutant.uncovered",
            serde_json::to_value(Site { mutation }),
            format!("No test runs {at}; its mutant {change} was not run"),
            format!(
                "Write a test that runs line {} of {file}.",
                mutant.span.line
            ),
        ),
        _ => return None,
    };
    Some(Note {
        kind,
        evidence: evidence.ok()?,
        title,
        summary,
    })
}

/// The operator's name as it serializes, such as `relational`.
fn operator(mutant: &Mutant) -> String {
    serde_json::to_value(mutant.operator)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The mutant's own site identity: its site and replacement, so each replacement of one site
/// is its own finding, stable wherever the site moves.
fn site_of(mutant: &Mutant) -> SiteHash {
    mutant.site_hash.refined(&mutant.to)
}
