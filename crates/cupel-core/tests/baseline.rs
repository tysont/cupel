// ABOUTME: The baseline test run (ADR-076) over synthetic runner producers: produced once before any check
// ABOUTME: whose needs reach `TestResults`, a red suite refusing the run, pins, detection, and upstream facts.
mod support;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use cupel_core::coverage::CoverageMap;
use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{
    Availability, Check, CheckCtx, CheckRegistry, CheckResult, CheckStatus, CostClass, Ctx,
    EventSink, FactKind, Facts, LangId, Langs, Manifest, Outcome, Plan, ProduceCtx, Producer,
    ProducerId, ProducerRegistry, Result, RunnerInvocation, Scheduler, Scope, Score, SkipReason,
    TestOutcome, TestResult, TestResults, ToolId, Unevaluable, Writer,
};

/// A runner producer for rust: one test per project root it is given, named after the root,
/// with `outcome`; counts its runs.
struct Runner {
    id: &'static str,
    detected: bool,
    installed: bool,
    outcome: TestOutcome,
    runs: Arc<AtomicUsize>,
}

impl Producer<TestResults> for Runner {
    fn id(&self) -> ProducerId {
        ProducerId::from(self.id)
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![LangId::from("rust")])
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    fn cost(&self) -> CostClass {
        CostClass::Standard
    }

    fn availability(&self, _: &Ctx) -> Availability {
        if self.installed {
            Availability::Ready
        } else {
            Availability::MissingTool {
                name: ToolId::from("fake-runner"),
                install: None,
            }
        }
    }

    fn detected(&self, _: &Ctx) -> bool {
        self.detected
    }

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<TestResults> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        let roots = ctx
            .upstream
            .get::<ProjectInfo>()
            .map(|info| info.roots.clone());
        let tests = roots
            .unwrap_or_default()
            .into_iter()
            .map(|root| TestResult {
                id: format!("{} {}", self.id, root.path),
                file: None,
                name: root.path,
                outcome: self.outcome,
                duration_ms: Some(1),
                output: None,
            })
            .collect();
        let invocation = RunnerInvocation {
            dir: ".".to_owned(),
            command: vec![self.id.to_owned()],
            exit_code: Some(0),
            completed: true,
            output: None,
        };
        Ok(TestResults::new(
            self.id(),
            vec![invocation],
            tests,
            &ctx.ctx.root,
        ))
    }
}

/// Project roots for rust: `crates/app`.
struct Roots;

impl Producer<ProjectInfo> for Roots {
    fn id(&self) -> ProducerId {
        ProducerId::from("lang.rust")
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![LangId::from("rust")])
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn cost(&self) -> CostClass {
        CostClass::Quick
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(&self, _: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<ProjectInfo> {
        Ok(ProjectInfo {
            roots: vec![ProjectRoot {
                path: "crates/app".to_owned(),
                lang: LangId::from("rust"),
                build_files: vec!["Cargo.toml".to_owned()],
                members: Vec::new(),
                packages: Vec::new(),
                skip_dirs: Vec::new(),
            }],
        })
    }
}

/// An engine for rust that needs `TestResults`, standing in for the mutation engine.
struct Engine;

impl Producer<CoverageMap> for Engine {
    fn id(&self) -> ProducerId {
        ProducerId::from("engine.test")
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![LangId::from("rust")])
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::TestResults]
    }

    fn cost(&self) -> CostClass {
        CostClass::Deep
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(&self, _: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<CoverageMap> {
        Ok(CoverageMap::default())
    }
}

/// A check over `needs` whose score is the number of tests the baseline ran (0 without one).
struct Counts(Manifest);

impl Check for Counts {
    fn manifest(&self) -> &Manifest {
        &self.0
    }

    fn plan(&self, _: &CheckCtx, _: &Scope) -> Result<Plan> {
        Ok(Plan::estimate(CostClass::Quick, 1))
    }

    fn run(
        &self,
        _: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        _: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        let tests = facts
            .get::<TestResults>()
            .map_or(0, |results| results.tests.len());
        Ok(CheckResult {
            id: self.0.id.clone(),
            status: CheckStatus::Pass,
            tier: self.0.tier,
            cost: self.0.cost,
            score: Some(Score {
                value: f64::from(u32::try_from(tests).unwrap()),
                unit: "tests".to_owned(),
                n: 1,
            }),
            metrics: std::collections::BTreeMap::new(),
            delta: None,
            findings: 0,
            duration_ms: 0,
            skipped: None,
            oracle: None,
            notes: Vec::new(),
        })
    }
}

fn counts(id: &str, needs: &[FactKind]) -> Box<dyn Check> {
    Box::new(Counts(support::manifest(id, needs, &[])))
}

/// The runners a world registers, in registration order: `(id, detected, installed, outcome)`.
type Runners<'a> = &'a [(&'static str, bool, bool, TestOutcome)];

/// Runs `checks` on rust over `runners`, with `cupel.toml` holding `toml`; the result and the
/// number of runner runs.
fn run(checks: Vec<Box<dyn Check>>, runners: Runners<'_>, toml: &str) -> (Result<Outcome>, usize) {
    let dir = tempfile::tempdir().unwrap();
    let config = support::config_at(dir.path(), toml);
    let runs = Arc::new(AtomicUsize::new(0));
    let mut producers = ProducerRegistry::default();
    producers.register::<ProjectInfo, _>(Roots);
    producers.register::<CoverageMap, _>(Engine);
    for (id, detected, installed, outcome) in runners {
        producers.register::<TestResults, _>(Runner {
            id,
            detected: *detected,
            installed: *installed,
            outcome: *outcome,
            runs: Arc::clone(&runs),
        });
    }
    let mut registry = CheckRegistry::default();
    for check in checks {
        registry.register(check).unwrap();
    }
    let outcome = scheduled(dir.path(), &config, &registry, &producers);
    (outcome, runs.load(Ordering::SeqCst))
}

fn scheduled(
    root: &Path,
    config: &cupel_core::config::Config,
    checks: &CheckRegistry,
    producers: &ProducerRegistry,
) -> Result<Outcome> {
    let writer = Writer::new(root, false).unwrap();
    let job = support::job(&writer, config, checks, &["rust"]);
    Scheduler { checks, producers }.run(&job, &mut Vec::new())
}

const GREEN: Runners<'static> = &[("tool.alpha", true, true, TestOutcome::Passed)];
const RED: Runners<'static> = &[("tool.alpha", true, true, TestOutcome::Failed)];

/// Each check's ID, status, and score value.
fn results(outcome: &Outcome) -> Vec<(String, CheckStatus, Option<f64>)> {
    outcome
        .checks
        .iter()
        .map(|check| {
            (
                check.id.to_string(),
                check.status,
                check.score.as_ref().map(|score| score.value),
            )
        })
        .collect()
}

fn row(id: &str, status: CheckStatus, score: Option<f64>) -> (String, CheckStatus, Option<f64>) {
    (id.to_owned(), status, score)
}

#[test]
fn a_green_baseline_runs_once_and_serves_every_check_that_needs_it() {
    let checks = vec![
        counts("t.one", &[FactKind::TestResults]),
        counts("t.two", &[FactKind::TestResults]),
    ];
    let (outcome, runs) = run(checks, GREEN, "");
    let outcome = outcome.unwrap();
    assert_eq!(
        results(&outcome),
        [
            row("t.one", CheckStatus::Pass, Some(1.0)),
            row("t.two", CheckStatus::Pass, Some(1.0)),
        ]
    );
    let stored: Vec<(String, usize)> = outcome
        .test_results
        .iter()
        .map(|(lang, results)| (lang.to_string(), results.tests.len()))
        .collect();
    assert_eq!((runs, stored), (1, vec![("rust".to_owned(), 1)]));
}

#[test]
fn the_runner_receives_the_project_roots_it_needs() {
    let (outcome, _) = run(vec![counts("t.one", &[FactKind::TestResults])], GREEN, "");
    let ids: Vec<String> = outcome.unwrap().test_results[0]
        .1
        .tests
        .iter()
        .map(|test| test.id.clone())
        .collect();
    assert_eq!(ids, ["tool.alpha crates/app"]);
}

#[test]
fn a_red_baseline_refuses_the_run_before_any_check_runs() {
    let (outcome, runs) = run(
        vec![
            counts("t.cheap", &[FactKind::ProjectInfo]),
            counts("t.one", &[FactKind::TestResults]),
        ],
        RED,
        "",
    );
    let outcome = outcome.unwrap();
    let Unevaluable { lang, results } = outcome.refused.unwrap();
    let failing: Vec<&str> = results.failures().map(|test| test.id.as_str()).collect();
    assert_eq!(
        (lang.as_str(), failing, runs, outcome.checks.len()),
        ("rust", vec!["tool.alpha crates/app"], 1, 0)
    );
}

#[test]
fn a_check_whose_producer_needs_test_results_triggers_the_baseline() {
    let (outcome, runs) = run(vec![counts("t.engine", &[FactKind::CoverageMap])], RED, "");
    let refused = outcome.unwrap().refused.map(|red| red.lang.to_string());
    assert_eq!((refused, runs), (Some("rust".to_owned()), 1));
}

#[test]
fn no_suite_runs_when_no_selected_check_needs_test_results() {
    let (outcome, runs) = run(vec![counts("t.cheap", &[FactKind::ProjectInfo])], RED, "");
    assert_eq!(
        (results(&outcome.unwrap()), runs),
        (vec![row("t.cheap", CheckStatus::Pass, Some(0.0))], 0)
    );
}

/// A check's skip reason and missing fact; `None` when it ran.
type Skip = Option<(SkipReason, Option<FactKind>)>;

#[test]
fn a_baseline_that_ran_no_test_skips_the_checks_that_reach_it_as_unavailable() {
    let skipped_only: Runners<'static> = &[("tool.alpha", true, true, TestOutcome::Skipped)];
    let (outcome, runs) = run(
        vec![
            counts("t.cheap", &[FactKind::ProjectInfo]),
            counts("t.engine", &[FactKind::CoverageMap]),
            counts("t.one", &[FactKind::TestResults]),
        ],
        skipped_only,
        "",
    );
    let outcome = outcome.unwrap();
    let skips: Vec<(String, Skip)> = outcome
        .checks
        .iter()
        .map(|check| {
            let skip = check.skipped.as_ref();
            (
                check.id.to_string(),
                skip.map(|skipped| (skipped.reason, skipped.fact)),
            )
        })
        .collect();
    let unavailable = Some((SkipReason::Unavailable, Some(FactKind::TestResults)));
    assert_eq!(
        (skips, runs, outcome.interrupted, outcome.refused.is_none()),
        (
            vec![
                ("t.cheap".to_owned(), None),
                ("t.engine".to_owned(), unavailable),
                ("t.one".to_owned(), unavailable),
            ],
            1,
            false,
            true
        )
    );
}

/// The runner a run of one `TestResults` check used, under `runners` and `toml`.
fn runner_used(runners: Runners<'_>, toml: &str) -> String {
    let (outcome, _) = run(
        vec![counts("t.one", &[FactKind::TestResults])],
        runners,
        toml,
    );
    outcome.unwrap().test_results[0].1.runner.to_string()
}

const TWO: Runners<'static> = &[
    ("tool.alpha", false, true, TestOutcome::Passed),
    ("tool.beta", true, true, TestOutcome::Passed),
];

#[test]
fn a_detected_runner_is_preferred_and_a_pin_overrides_detection() {
    let used = [
        runner_used(TWO, ""),
        runner_used(TWO, "[lang.rust]\nrunner = \"alpha\"\n"),
        runner_used(TWO, "[lang.rust]\nrunner = \"tool.alpha\"\n"),
    ];
    assert_eq!(used, ["tool.beta", "tool.alpha", "tool.alpha"]);
}

/// The skip `t.one` (which needs `TestResults`) records under `runners` and `toml`.
fn skipped(runners: Runners<'_>, toml: &str) -> (SkipReason, Option<String>) {
    let (outcome, runs) = run(
        vec![counts("t.one", &[FactKind::TestResults])],
        runners,
        toml,
    );
    assert_eq!(runs, 0);
    let skipped = outcome.unwrap().checks[0].skipped.clone().unwrap();
    (skipped.reason, skipped.tool.map(|tool| tool.to_string()))
}

#[test]
fn a_pin_naming_no_runner_is_unconfigured_and_a_missing_tool_unavailable() {
    let missing: Runners<'static> = &[("tool.alpha", true, false, TestOutcome::Passed)];
    assert_eq!(
        [
            skipped(GREEN, "[lang.rust]\nrunner = \"gamma\"\n"),
            skipped(missing, ""),
        ],
        [
            (SkipReason::Unconfigured, None),
            (SkipReason::Unavailable, Some("fake-runner".to_owned())),
        ]
    );
}

#[test]
fn pins_are_keyed_by_fact_kind() {
    let ctx = Ctx {
        lang: [(
            LangId::from("rust"),
            serde_json::json!({"runner": "unittest"})
                .as_object()
                .unwrap()
                .clone(),
        )]
        .into(),
        ..Ctx::new(std::env::temp_dir())
    };
    let rust = LangId::from("rust");
    assert_eq!(
        [
            ctx.pin(FactKind::TestResults, &rust),
            ctx.pin(FactKind::Symbols, &rust),
            ctx.pin(FactKind::TestResults, &LangId::from("go")),
        ],
        [Some("unittest"), None, None]
    );
}

#[test]
fn mutation_outcomes_are_pinned_by_mutator() {
    let ctx = Ctx {
        lang: [(
            LangId::from("java"),
            serde_json::json!({"mutator": "plugin.pit", "mutation": {"set": "extended"}})
                .as_object()
                .unwrap()
                .clone(),
        )]
        .into(),
        ..Ctx::new(std::env::temp_dir())
    };
    assert_eq!(FactKind::MutationOutcomes.pin_key(), Some("mutator"));
    assert_eq!(
        ctx.pin(FactKind::MutationOutcomes, &LangId::from("java")),
        Some("plugin.pit")
    );
}
