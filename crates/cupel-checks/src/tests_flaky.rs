// ABOUTME: `tests.flaky`: tests whose outcome changes across the flake engine's reruns (ADR-078), read from
// ABOUTME: TestReruns and located through TestFrames: flaky in one order, or dependent on the order.
use std::collections::BTreeMap;

use cupel_core::syntax::{Span, TestFrames};
use cupel_core::{
    Action, ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink,
    Evidence, FactKind, Facts, Finding, FindingId, FindingKindSpec, Location, Manifest, MetricSpec,
    Plan, Result, RunOrder, Scope, Score, Severity, Stability, TestOutcome, TestRerun, TestReruns,
    Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::manifest::{evidence, kind, metric};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "tests.flaky";

/// The check takes no settings; the rerun count is the engine's (`[lang.<id>] flake_runs`).
const LIMITS: Limits = Limits::new(&[]);

/// Evidence of `test.flaky`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Flaky {
    /// The test's ID, as its runner names it.
    test: String,
    /// The runner that reran it.
    runner: String,
    /// Reruns in which the test passed, failed, or errored.
    reruns: u32,
    passed: u32,
    /// Failed or errored.
    failed: u32,
    /// The test's outcome in each rerun, in rerun order; `absent` where it did not run.
    outcomes: Vec<String>,
    /// Two reruns (by index) that ran the tests in the same order and disagreed.
    same_order: Vec<usize>,
}

/// Evidence of `test.order-dependent`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OrderDependent {
    /// The test's ID, as its runner names it.
    test: String,
    /// The runner that reran it.
    runner: String,
    /// Reruns in which the test passed, failed, or errored.
    reruns: u32,
    passed: u32,
    /// Failed or errored.
    failed: u32,
    /// The test's outcome in each rerun, in rerun order; `absent` where it did not run.
    outcomes: Vec<String>,
    /// The order each rerun asked for: `declared`, `reversed`, or `shuffled`.
    orders: Vec<String>,
    /// Tests that ran before it in every rerun it failed and after it in some rerun it passed:
    /// the likely polluters. Empty when the runner does not report its order.
    preceded_by: Vec<String>,
}

pub struct TestsFlaky {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(TestsFlaky::new())));

impl TestsFlaky {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(2).expect("tier 2 exists"),
                cost: CostClass::Standard,
                needs: vec![FactKind::TestReruns, FactKind::TestFrames],
                wants: Vec::new(),
                findings: finding_kinds(),
                metrics: metrics(),
                stability: Stability::Experimental,
                summary: "Reruns the tests several times with shuffled order, varied seeds, and \
                          varied parallelism; reports tests whose outcome changes."
                    .to_owned(),
            },
        }
    }
}

fn finding_kinds() -> Vec<FindingKindSpec> {
    vec![
        kind::<Flaky>(
            "test.flaky",
            ActionKind::InvestigateFlake,
            "The test passed and failed in reruns that ran the tests in the same order.",
        ),
        kind::<OrderDependent>(
            "test.order-dependent",
            ActionKind::InvestigateFlake,
            "The test's outcome follows the order the tests ran in: the same in every rerun of \
             one order, different between orders.",
        ),
    ]
}

fn metrics() -> Vec<MetricSpec> {
    vec![
        metric(
            "flaky",
            "Tests whose outcome changed between reruns of the same order.",
        ),
        metric(
            "order_dependent",
            "Tests whose outcome changed only with the order the tests ran in.",
        ),
    ]
}

impl Check for TestsFlaky {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        LIMITS.read(&self.manifest.id, &ctx.config)?;
        Ok(outcome::plan_files(&self.manifest, scope))
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        LIMITS.read(&self.manifest.id, &ctx.config)?;
        let reruns = facts.get::<TestReruns>()?;
        let frames = facts.get::<TestFrames>()?;
        let histories = histories(reruns);
        let mut findings = Vec::new();
        for (id, history) in &histories {
            if let Some(finding) = self.classify(reruns, frames, id, history) {
                findings.push(finding);
            }
        }
        let count = |kind: &str| findings.iter().filter(|f| f.kind == kind).count() as u64;
        let (flaky, ordered) = (count("test.flaky"), count("test.order-dependent"));
        let reported = findings.len() as u64;
        for finding in findings {
            sink.emit(Event::Finding(Box::new(finding)));
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "test counts stay far below 2^52, where f64 is exact"
        )]
        let metrics = BTreeMap::from([
            ("flaky".to_owned(), flaky as f64),
            ("order_dependent".to_owned(), ordered as f64),
        ]);
        #[expect(
            clippy::cast_precision_loss,
            reason = "test counts stay far below 2^52, where f64 is exact"
        )]
        let score = Score {
            value: reported as f64,
            unit: "unstable".to_owned(),
            n: histories.len() as u64,
        };
        Ok(outcome::result(
            &self.manifest,
            outcome::status(reported > 0),
            Some(score),
            metrics,
            reported,
        ))
    }

    /// Unstable tests add up across languages.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }

    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }
}

/// What one test did across the reruns.
struct History<'a> {
    /// Its outcome in each rerun; `None` where it did not run.
    outcomes: Vec<Option<TestOutcome>>,
    file: Option<&'a str>,
    name: &'a str,
}

/// Every test of every rerun, by ID.
fn histories(reruns: &TestReruns) -> BTreeMap<&str, History<'_>> {
    let mut histories: BTreeMap<&str, History<'_>> = BTreeMap::new();
    for (index, run) in reruns.runs.iter().enumerate() {
        for test in &run.tests {
            let history = histories.entry(&test.id).or_insert_with(|| History {
                outcomes: vec![None; reruns.runs.len()],
                file: test.file.as_deref(),
                name: &test.name,
            });
            history.outcomes[index] = Some(test.outcome);
        }
    }
    histories
}

impl History<'_> {
    /// Whether the test failed in rerun `index`; `None` where it was skipped or did not run.
    fn failed(&self, index: usize) -> Option<bool> {
        match self.outcomes[index]? {
            TestOutcome::Skipped => None,
            outcome => Some(outcome.is_failure()),
        }
    }

    /// The reruns that decided the test: (index, failed).
    fn decided(&self) -> Vec<(usize, bool)> {
        (0..self.outcomes.len())
            .filter_map(|index| Some((index, self.failed(index)?)))
            .collect()
    }

    fn outcome_names(&self) -> Vec<String> {
        self.outcomes
            .iter()
            .map(|outcome| outcome.map_or("absent", TestOutcome::as_str).to_owned())
            .collect()
    }
}

/// Two decided reruns with one order key whose outcomes differ.
fn same_order_disagreement(runs: &[TestRerun], decided: &[(usize, bool)]) -> Option<Vec<usize>> {
    decided
        .iter()
        .enumerate()
        .find_map(|(at, (first, failed))| {
            decided[at + 1..]
                .iter()
                .find(|(second, other)| {
                    other != failed && runs[*first].order_key() == runs[*second].order_key()
                })
                .map(|(second, _)| vec![*first, *second])
        })
}

/// The tests that ran before `id` in every rerun it failed and after it in some rerun it
/// passed, in the order of the first failing rerun; empty when a rerun did not report its order.
fn polluters(runs: &[TestRerun], decided: &[(usize, bool)], id: &str) -> Vec<String> {
    let before = |index: usize| -> Option<Vec<&str>> {
        let executed = runs[index].executed.as_ref()?;
        let at = executed.iter().position(|ran| ran == id)?;
        Some(executed[..at].iter().map(String::as_str).collect())
    };
    let mut failing = decided.iter().filter(|(_, failed)| *failed);
    let Some(first) = failing.next().and_then(|(index, _)| before(*index)) else {
        return Vec::new();
    };
    let mut others = Vec::new();
    for (index, _) in failing {
        let Some(ran) = before(*index) else {
            return Vec::new();
        };
        others.push(ran);
    }
    let mut passing = Vec::new();
    for (index, _) in decided.iter().filter(|(_, failed)| !*failed) {
        let Some(ran) = before(*index) else {
            return Vec::new();
        };
        passing.push(ran);
    }
    first
        .into_iter()
        .filter(|candidate| others.iter().all(|ran| ran.contains(candidate)))
        .filter(|candidate| passing.iter().any(|ran| !ran.contains(candidate)))
        .map(str::to_owned)
        .collect()
}

/// Counts of decided reruns: (reruns, passed, failed).
fn tally(decided: &[(usize, bool)]) -> (u32, u32, u32) {
    let failed = decided.iter().filter(|(_, failed)| *failed).count();
    let all = u32::try_from(decided.len()).unwrap_or(u32::MAX);
    let failed = u32::try_from(failed).unwrap_or(u32::MAX);
    (all, all - failed, failed)
}

/// What a finding says, before identity and location.
struct Said {
    kind: &'static str,
    title: String,
    summary: String,
    evidence: Value,
    confidence: f64,
}

impl TestsFlaky {
    /// The finding about test `id`, when its outcome changed across the reruns.
    fn classify(
        &self,
        reruns: &TestReruns,
        frames: &TestFrames,
        id: &str,
        history: &History<'_>,
    ) -> Option<Finding> {
        let decided = history.decided();
        let failures = decided.iter().filter(|(_, failed)| *failed).count();
        if failures == 0 || failures == decided.len() {
            return None;
        }
        let said = if let Some(same_order) = same_order_disagreement(&reruns.runs, &decided) {
            flaky(reruns, id, history, &decided, same_order)
        } else {
            order_dependent(reruns, id, history, &decided)
        };
        Some(self.finding(said, id, location(frames, history), reruns.seed))
    }

    fn finding(&self, said: Said, id: &str, location: Option<Location>, seed: u64) -> Finding {
        let finding_id = FindingId::for_subject(&self.manifest.id, said.kind, id);
        let evidence = match said.evidence {
            Value::Object(fields) => fields,
            _ => serde_json::Map::new(),
        };
        Finding {
            suppress: format!("cupel suppress {finding_id} --reason '…'"),
            repro: format!("cupel check --only {} --seed {seed}", self.manifest.id),
            id: finding_id,
            check: self.manifest.id.clone(),
            kind: said.kind.to_owned(),
            severity: Severity::Warn,
            confidence: said.confidence,
            tier: self.manifest.tier,
            symbol: None,
            location,
            title: said.title,
            detail: None,
            evidence: Evidence(evidence),
            action: Action {
                kind: ActionKind::InvestigateFlake,
                priority: 2,
                summary: said.summary,
                hints: serde_json::Map::new(),
            },
            new_since_baseline: false,
            suppressed: false,
        }
    }
}

/// `test.flaky`: reruns `same_order` ran the tests in one order and disagreed about `id`.
fn flaky(
    reruns: &TestReruns,
    id: &str,
    history: &History<'_>,
    decided: &[(usize, bool)],
    same_order: Vec<usize>,
) -> Said {
    let name = history.name;
    let (count, passed, failed) = tally(decided);
    Said {
        kind: "test.flaky",
        title: format!("`{name}` passed and failed in reruns of the same order"),
        summary: format!(
            "Find what `{name}` depends on besides its code and the test order: time, \
             randomness, concurrency, or the environment; make it deterministic."
        ),
        evidence: evidence(&Flaky {
            test: id.to_owned(),
            runner: reruns.runner.to_string(),
            reruns: count,
            passed,
            failed,
            outcomes: history.outcome_names(),
            same_order,
        }),
        confidence: 0.95,
    }
}

/// `test.order-dependent`: every order the reruns asked for agreed with itself about `id`.
fn order_dependent(
    reruns: &TestReruns,
    id: &str,
    history: &History<'_>,
    decided: &[(usize, bool)],
) -> Said {
    let name = history.name;
    let (count, passed, failed) = tally(decided);
    let preceded_by = polluters(&reruns.runs, decided, id);
    let confidence = if preceded_by.is_empty() { 0.7 } else { 0.9 };
    Said {
        kind: "test.order-dependent",
        title: format!("`{name}` passes or fails depending on the test order"),
        summary: format!(
            "Make `{name}` independent of the tests before it: reset the shared state they \
             leave behind, or set up what it relies on itself."
        ),
        evidence: evidence(&OrderDependent {
            test: id.to_owned(),
            runner: reruns.runner.to_string(),
            reruns: count,
            passed,
            failed,
            outcomes: history.outcome_names(),
            orders: reruns
                .runs
                .iter()
                .map(|run| order_name(run.order))
                .collect(),
            preceded_by,
        }),
        confidence,
    }
}

fn order_name(order: RunOrder) -> String {
    match order {
        RunOrder::Declared => "declared",
        RunOrder::Reversed => "reversed",
        RunOrder::Shuffled => "shuffled",
    }
    .to_owned()
}

/// Where the test is: its case in `TestFrames` (in its file, when the runner names one, else
/// the one case anywhere with its name); else line 1 of its file; else nowhere.
fn location(frames: &TestFrames, history: &History<'_>) -> Option<Location> {
    let cases: Vec<(&str, &Span)> = frames
        .files
        .iter()
        .filter(|file| history.file.is_none_or(|known| known == file.file))
        .flat_map(|file| {
            file.cases
                .iter()
                .filter(|case| names_match(history.name, &case.name))
                .map(|case| (file.file.as_str(), &case.span))
        })
        .collect();
    match (cases.as_slice(), history.file) {
        ([(file, span)], _) => Some(Location {
            file: (*file).to_owned(),
            line: span.line,
            col: Some(span.col),
            end_line: span.end_line,
            end_col: Some(span.end_col),
        }),
        (_, Some(file)) => Some(Location {
            file: file.to_owned(),
            line: 1,
            col: None,
            end_line: 1,
            end_col: None,
        }),
        _ => None,
    }
}

/// Whether the runner's name of a test names the case `case`: equal, or ending in it after a
/// separator (`tests::case`, `Suite > case`, `test_case (module.Class.test_case)` excepted).
fn names_match(runner: &str, case: &str) -> bool {
    runner == case
        || runner
            .strip_suffix(case)
            .is_some_and(|head| head.ends_with(|c: char| !(c.is_alphanumeric() || c == '_')))
}
