// ABOUTME: `tests.mutation`: scores MutationOutcomes, whoever produced them (ADR-080): surviving mutants as failures at their
// ABOUTME: site; timeouts, runs over the memory budget, uncovered sites, slow tests left out, equivalents, fallbacks as notes.
use std::collections::{BTreeMap, BTreeSet, HashMap};

use cupel_core::config::{ConfigError, MemoryLimit, Table, TimeLimit};
use cupel_core::syntax::{SymbolDef, SymbolKey, Symbols, TestConventions};
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, ExcludedTest,
    FactKind, Facts, Finding, Manifest, Measurement, MetricPer, MetricSpec, Mutant, MutantOutcome,
    MutationOutcomes, MutationStrategy, Plan, ProtectionMetric, Result, Scope, Score, SettingSpec,
    Severity, SiteHash, Stability, SymbolHash, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::finding::{SubjectFinding, SymbolFinding, with_nearest_test};
use crate::manifest::{kind, metric, protection, ranked, setting};
use crate::metrics::{scored, sum, symbol_target, weighted_mean};
use crate::outcome;

const ID: &str = "tests.mutation";

/// The check's settings, the cap on a mutant's run (ADR-147) and the memory budget of the runs
/// together (ADR-148), which configure the engine's fact; everything else about how mutants
/// run is the engine's (`[lang.<id>] mutation`).
const MAX_MUTANT_TIMEOUT: &str = "max_mutant_timeout";
const MAX_MUTANT_MEMORY: &str = "max_mutant_memory";

/// The cap and the budget when none is configured; the engine's defaults, which a test holds
/// equal.
const DEFAULT_MAX_MUTANT_TIMEOUT: &str = "120s";
const DEFAULT_MAX_MUTANT_MEMORY: &str = "50%";

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

/// Evidence of `mutant.timeout`, `mutant.out-of-memory`, and `mutant.uncovered`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Site {
    mutation: Mutation,
}

/// Evidence of `mutant.slow-test-excluded`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SlowTest {
    /// The runner's ID of the test.
    test: String,
    /// Its file, when the runner names one.
    file: Option<String>,
    /// How long it ran in the baseline, in milliseconds.
    baseline_ms: u64,
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
                "mutant.out-of-memory",
                ActionKind::Review,
                "The tests held more memory than the run's share of max_mutant_memory against \
                 the mutant and were killed (ADR-148).",
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
            kind::<SlowTest>(
                "mutant.slow-test-excluded",
                ActionKind::Review,
                "A test slower than 60 s in the baseline was left out of the mutants' runs \
                 that per-test selection did not narrow (ADR-147).",
            ),
        ],
        metrics: metrics(),
        settings: settings(),
        stability: Stability::Experimental,
        summary: "Mutates the code in scope and runs the tests that cover each mutant; a \
                  mutant every test passes against survived. Score: killed / (killed + \
                  survived + uncovered): a site no test runs is not run and counts as missed \
                  (ADR-141). Outcomes of unchanged code and tests are reused from earlier runs."
            .to_owned(),
    }
}

/// The settings this check declares, each configuring the engine's fact (ADR-147 §2).
fn settings() -> Vec<SettingSpec> {
    [
        (
            MAX_MUTANT_TIMEOUT,
            DEFAULT_MAX_MUTANT_TIMEOUT,
            "The longest a mutant's run may take, as a duration such as 90s or 5m; a run that \
             reaches it is a timeout, whatever timeout_factor allows.",
        ),
        (
            MAX_MUTANT_MEMORY,
            DEFAULT_MAX_MUTANT_MEMORY,
            "The resident memory the mutant runs may hold together, as a size such as 3g or \
             512m or a share of physical memory such as 50%; each slot's run gets an equal \
             part, and a run over it is killed and is out-of-memory.",
        ),
    ]
    .into_iter()
    .map(|(name, default, summary)| SettingSpec {
        configures: Some(FactKind::MutationOutcomes),
        ..setting(name, Value::from(default), summary)
    })
    .collect()
}

/// The metrics this check reports: gate keys, then what `survey` ranks.
fn metrics() -> Vec<MetricSpec> {
    let mut metrics: Vec<MetricSpec> = [
        (
            "score",
            "Killed / (killed + survived + uncovered), the score; 1 when nothing was scored.",
        ),
        ("killed", "Mutants a test failed against."),
        (
            "survived",
            "Mutants every test passed against, not counting possibly equivalent ones.",
        ),
        ("timeout", "Mutants whose tests ran past their timeout."),
        (
            "out_of_memory",
            "Mutants whose tests held more than their share of max_mutant_memory and were killed.",
        ),
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
    metrics.extend([
        ranked(
            "survivors",
            MetricPer::Symbol,
            "Mutants of the function that survived.",
        ),
        protection(
            "mutation_score",
            ProtectionMetric::MutationScore,
            "Killed / (killed + survived + uncovered) over the function's mutants; absent when none was scored.",
        ),
        protection(
            "tests",
            ProtectionMetric::Tests,
            "Distinct tests run against the function's mutants.",
        ),
    ]);
    metrics
}

impl Check for TestsMutation {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        read_settings(&ctx.config)?;
        Ok(outcome::plan_symbols(&self.manifest, scope))
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        read_settings(&ctx.config)?;
        let outcomes = facts.get::<MutationOutcomes>()?;
        let symbols: HashMap<SymbolKey, &SymbolDef> = facts
            .get::<Symbols>()?
            .symbols
            .iter()
            .map(|symbol| (symbol.key(), symbol))
            .collect();
        let conventions = facts.get::<TestConventions>().ok();
        let mut tally = Tally::default();
        let mut by_symbol: BTreeMap<&SymbolKey, Protection<'_>> = BTreeMap::new();
        for mutant in outcomes
            .mutants
            .iter()
            .filter(|mutant| ctx.scope.includes(&mutant.symbol))
        {
            let Some(symbol) = symbols.get(&mutant.symbol) else {
                continue;
            };
            tally.count(mutant);
            by_symbol
                .entry(&mutant.symbol)
                .or_insert_with(|| Protection::of(symbol, mutant.symbol_hash.as_ref()))
                .count(mutant);
            if let Some(finding) = self.finding(mutant, symbol, conventions) {
                tally.findings += 1;
                sink.emit(Event::Finding(Box::new(finding)));
            }
        }
        for protection in by_symbol.values() {
            protection.measure(sink);
        }
        for test in &outcomes.excluded {
            tally.findings += 1;
            sink.emit(Event::Finding(Box::new(self.slow_test(test))));
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

/// How well one function's tests protect it, from its mutants (ADR-132).
struct Protection<'a> {
    symbol: &'a SymbolDef,
    hash: Option<&'a SymbolHash>,
    killed: u32,
    survived: u32,
    uncovered: u32,
    tests: BTreeSet<&'a str>,
}

impl<'a> Protection<'a> {
    fn of(symbol: &'a SymbolDef, hash: Option<&'a SymbolHash>) -> Self {
        Self {
            symbol,
            hash,
            killed: 0,
            survived: 0,
            uncovered: 0,
            tests: BTreeSet::new(),
        }
    }

    fn count(&mut self, mutant: &'a Mutant) {
        self.survived += u32::from(surviving(mutant));
        self.killed += u32::from(mutant.outcome == MutantOutcome::Killed);
        self.uncovered += u32::from(mutant.outcome == MutantOutcome::Uncovered);
        self.tests.extend(mutant.tests.iter().map(String::as_str));
    }

    /// Reports its survivors, its killed ratio when a mutant was scored (killed, survived, or
    /// uncovered), and its tests.
    fn measure(&self, sink: &mut dyn EventSink) {
        let scored = self.killed + self.survived + self.uncovered;
        let tests = u32::try_from(self.tests.len()).unwrap_or(u32::MAX);
        let values = [
            ("survivors", Some(f64::from(self.survived))),
            (
                "mutation_score",
                (scored > 0).then(|| f64::from(self.killed) / f64::from(scored)),
            ),
            ("tests", Some(f64::from(tests))),
        ];
        for (metric, value) in values {
            if let Some(value) = value {
                sink.measure(Measurement {
                    metric: metric.to_owned(),
                    value,
                    target: symbol_target(self.symbol, self.hash),
                });
            }
        }
    }
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

    /// The language's result: `fail` with a survivor, scored by the killed ratio over the
    /// killed, surviving, and uncovered mutants (ADR-141).
    fn result(&self, manifest: &Manifest) -> CheckResult {
        let (killed, survived) = (
            self.of(MutantOutcome::Killed),
            self.of(MutantOutcome::Survived),
        );
        let scored = killed + survived + self.of(MutantOutcome::Uncovered);
        let score = if scored == 0 {
            1.0
        } else {
            count(killed) / count(scored)
        };
        let mut metrics: BTreeMap<String, f64> = [
            MutantOutcome::Killed,
            MutantOutcome::Survived,
            MutantOutcome::Timeout,
            MutantOutcome::OutOfMemory,
            MutantOutcome::Uncovered,
            MutantOutcome::Unviable,
        ]
        .into_iter()
        .map(|outcome| (metric_name(outcome), count(self.of(outcome))))
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

/// An outcome's metric: its name with `_` for `-`, as a gate key is written.
fn metric_name(outcome: MutantOutcome) -> String {
    outcome.as_str().replace('-', "_")
}

/// Refuses any key of `config` but the cap and the budget, a cap that is not a duration, and
/// a budget that is neither a size nor a share.
fn read_settings(config: &Table) -> Result<()> {
    let refuse = |key: &str, reason: String| ConfigError::CheckSetting {
        check: ID.to_owned(),
        key: key.to_owned(),
        reason,
        valid: vec![MAX_MUTANT_TIMEOUT.to_owned(), MAX_MUTANT_MEMORY.to_owned()],
    };
    for (key, value) in config {
        let text = value.as_str();
        let (well_formed, expected) = match key.as_str() {
            MAX_MUTANT_TIMEOUT => (
                text.is_some_and(|text| text.parse::<TimeLimit>().is_ok()),
                "a duration such as \"90s\" or \"5m\"",
            ),
            MAX_MUTANT_MEMORY => (
                text.is_some_and(|text| text.parse::<MemoryLimit>().is_ok()),
                "a size such as \"3g\" or \"512m\", or a share such as \"50%\"",
            ),
            _ => return Err(refuse(key, "not a setting of this check".to_owned()).into()),
        };
        if !well_formed {
            return Err(refuse(key, format!("expected {expected}, got {value}")).into());
        }
    }
    Ok(())
}

impl TestsMutation {
    /// `mutant.slow-test-excluded` about `test`, left out of the whole-suite runs.
    fn slow_test(&self, test: &ExcludedTest) -> Finding {
        #[allow(
            clippy::cast_precision_loss,
            reason = "a test's duration in milliseconds stays far below 2^52"
        )]
        let seconds = test.baseline_ms as f64 / 1000.0;
        SubjectFinding {
            kind: "mutant.slow-test-excluded",
            subject: test.id.clone(),
            severity: Severity::Info,
            confidence: 1.0,
            title: format!(
                "Slow test left out of mutant runs: `{}` ({seconds:.1} s in the baseline)",
                test.id
            ),
            summary: format!(
                "Make `{}` run in under 60 s, or let per-test selection narrow the mutants that \
                 it covers (a per-test map, cached or at --depth deep), so they run it.",
                test.id
            ),
            evidence: serde_json::to_value(SlowTest {
                test: test.id.clone(),
                file: test.file.clone(),
                baseline_ms: test.baseline_ms,
            })
            .unwrap_or(Value::Null),
            location: None,
            repro: format!("cupel check --only {ID}"),
        }
        .finding(&self.manifest)
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
/// equivalent, timed out, killed over its memory budget, or uncovered; none for one killed
/// or unviable.
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
        (MutantOutcome::OutOfMemory, _) => (
            "mutant.out-of-memory",
            serde_json::to_value(Site { mutation }),
            format!("Mutant ran out of memory: {change} at {at}"),
            format!(
                "Review {change} at {at}: the tests held more than their share of \
                 `max_mutant_memory` against it and were killed; raise it or lower \
                 `mutation.jobs` if the tests honestly need more."
            ),
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
