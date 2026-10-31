// ABOUTME: The flake engine (ADR-078): its schedule from the seed, what it does with no baseline or a runner it
// ABOUTME: cannot rerun, cancellation, and real reruns of each runner on the tests.flaky fixtures (skipped when absent).
use std::path::Path;

use cupel_adapt::flake::{DEFAULT_RUNS, rerun_seed, schedule};
use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{
    Availability, Cancellation, Ctx, Error, FactKind, Lang, LangId, ProduceCtx, Produced,
    ProducerId, ProducerRegistry, RegisteredProducer, RunOrder, RunOrdering, Scope, TestOutcome,
    TestReruns, TestResults, Writer,
};
use cupel_fixtures::copy_repo;
use cupel_fixtures::matrix::Registries;
use cupel_fixtures::own_tmpdir::ran_in_own_tmpdir;
use serde_json::json;

use TestOutcome::{Failed, Passed};

/// The seed the matrix goldens run with, whose shuffles put the polluter before its victim for
/// every shuffling runner.
const SEED: u64 = cupel_fixtures::matrix::SEED;

fn registry() -> &'static ProducerRegistry {
    &Registries::shared().producers
}

/// The registered language `id`.
fn lang(id: &str) -> Lang {
    Registries::shared().lang(id)
}

fn producer<'a>(
    registry: &'a ProducerRegistry,
    kind: FactKind,
    id: &str,
    lang: &str,
) -> &'a dyn RegisteredProducer {
    registry
        .producers_for(kind, &self::lang(lang))
        .find(|producer| producer.id().as_str() == id)
        .unwrap_or_else(|| panic!("{id} produces {kind} for {lang}"))
}

/// The context of producing at `root` with `seed`, the `[lang.<id>]` table `config`, and
/// `upstream` facts.
fn produce_ctx(
    root: &Path,
    seed: u64,
    config: serde_json::Value,
    upstream: Vec<Produced>,
) -> ProduceCtx {
    let mut ctx = ProduceCtx::new(
        Ctx::new(root.to_path_buf()),
        Writer::new(root, false).unwrap().confined(),
        Cancellation::new(),
    );
    ctx.seed = seed;
    ctx.runners = registry().runners();
    let serde_json::Value::Object(config) = config else {
        panic!("config is a table")
    };
    ctx.config = config;
    for fact in upstream {
        ctx.upstream.insert(fact.share());
    }
    ctx
}

fn project(lang: &str) -> Produced {
    Produced::new(ProjectInfo {
        roots: vec![ProjectRoot {
            path: ".".to_owned(),
            lang: LangId::from(lang),
            build_files: Vec::new(),
            members: Vec::new(),
            packages: if lang == "go" {
                vec!["./...".to_owned()]
            } else {
                Vec::new()
            },
            skip_dirs: Vec::new(),
        }],
    })
}

fn engine_produce(ctx: &ProduceCtx, lang: &str) -> cupel_core::Result<TestReruns> {
    let registry = registry();
    producer(registry, FactKind::TestReruns, "engine.flake", lang)
        .produce(ctx, &Scope::default(), &mut Vec::new())
        .map(|fact| fact.downcast::<TestReruns>().unwrap())
}

#[test]
fn the_schedule_follows_the_seed_alone() {
    let plan = schedule(42, 5);
    let orders: Vec<RunOrder> = plan.iter().map(|run| run.order).collect();
    assert_eq!(
        orders,
        [
            RunOrder::Declared,
            RunOrder::Declared,
            RunOrder::Reversed,
            RunOrder::Shuffled,
            RunOrder::Shuffled,
        ]
    );
    let parallelism: Vec<Option<u32>> = plan.iter().map(|run| run.parallelism).collect();
    assert_eq!(parallelism, [Some(1), Some(1), Some(1), None, Some(1)]);
    let seeds: Vec<u64> = plan.iter().map(|run| run.seed).collect();
    assert_eq!(seeds, (0..5).map(|k| rerun_seed(42, k)).collect::<Vec<_>>());
    assert_eq!(schedule(42, 5), plan);
    assert_ne!(schedule(43, 5)[0].seed, plan[0].seed);
    assert_ne!(plan[0].seed, plan[1].seed);
    assert_eq!(schedule(42, DEFAULT_RUNS).len(), 3);
}

#[test]
fn a_reversed_rerun_reverses_the_first_order_or_shuffles_without_one() {
    let first = vec!["a".to_owned(), "b".to_owned()];
    let planned = schedule(1, 3).remove(2);
    let explicit = planned.clone().resolved(true, Some(&first));
    assert_eq!(
        (explicit.order, explicit.explicit),
        (RunOrder::Reversed, vec!["b".to_owned(), "a".to_owned()])
    );
    let shuffled = planned.clone().resolved(false, Some(&first));
    assert_eq!(
        (shuffled.order, shuffled.explicit.len()),
        (RunOrder::Shuffled, 0)
    );
    assert_eq!(planned.resolved(true, None).order, RunOrder::Shuffled);
}

#[test]
fn the_engine_reruns_any_language_and_needs_the_baseline() {
    let registry = registry();
    for lang in ["rust", "go", "python", "javascript", "typescript"] {
        let engine = producer(registry, FactKind::TestReruns, "engine.flake", lang);
        assert_eq!(engine.needs(), [FactKind::TestResults]);
        assert_eq!(engine.wants(), [FactKind::ProjectInfo]);
    }
    let mut reruns: Vec<String> = ["rust", "go", "python", "javascript"]
        .into_iter()
        .flat_map(|lang| registry.producers_for(FactKind::TestResults, &self::lang(lang)))
        .map(RegisteredProducer::id)
        .filter(|id| {
            registry
                .runner(id)
                .is_some_and(|runner| runner.ordering() != RunOrdering::Fixed)
        })
        .map(|id| id.to_string())
        .collect();
    reruns.sort();
    assert_eq!(
        reruns,
        [
            "tool.cargo-test",
            "tool.go-test",
            "tool.jest",
            "tool.pytest",
            "tool.unittest",
            "tool.vitest"
        ]
    );
}

#[test]
fn with_no_baseline_the_engine_reruns_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = produce_ctx(dir.path(), 9, json!({}), Vec::new());
    let reruns = engine_produce(&ctx, "python").unwrap();
    assert_eq!(
        (reruns.runner.as_str(), reruns.seed, reruns.runs.len()),
        ("engine.flake", 9, 0)
    );
}

fn baseline_of(runner: &str) -> Produced {
    Produced::new(TestResults::new(
        ProducerId::from(runner),
        Vec::new(),
        Vec::new(),
        Path::new("."),
    ))
}

#[test]
fn a_runner_the_engine_cannot_rerun_is_an_error_naming_it() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = produce_ctx(
        dir.path(),
        1,
        json!({}),
        vec![baseline_of("tool.cargo-nextest")],
    );
    let error = engine_produce(&ctx, "rust").unwrap_err();
    let Error::InvalidValue { what, value, .. } = error else {
        panic!("{error:?}")
    };
    assert_eq!(
        (what, value.as_str()),
        ("runner for engine.flake", "tool.cargo-nextest")
    );
}

#[test]
fn flake_runs_sets_the_count_and_must_be_a_count() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = || vec![baseline_of("tool.pytest")];
    let none = produce_ctx(dir.path(), 1, json!({"flake_runs": 0}), upstream());
    assert_eq!(engine_produce(&none, "python").unwrap().runs.len(), 0);
    let bad = produce_ctx(dir.path(), 1, json!({"flake_runs": "x"}), upstream());
    assert!(matches!(
        engine_produce(&bad, "python"),
        Err(Error::InvalidValue {
            what: "flake_runs",
            ..
        })
    ));
}

#[test]
fn a_cancelled_run_stops_the_reruns() {
    let dir = tempfile::tempdir().unwrap();
    let mut ctx = produce_ctx(
        dir.path(),
        1,
        json!({}),
        vec![Produced::new(TestResults::new(
            ProducerId::from("tool.go-test"),
            vec![cupel_core::RunnerInvocation {
                dir: ".".to_owned(),
                command: vec!["go".to_owned(), "test".to_owned(), "-json".to_owned()],
                exit_code: Some(0),
                completed: true,
                output: None,
            }],
            Vec::new(),
            dir.path(),
        ))],
    );
    let cancel = Cancellation::new();
    cancel.cancel();
    ctx.cancel = cancel;
    assert_eq!(engine_produce(&ctx, "go").unwrap_err(), Error::Cancelled);
}

/// The baseline of `runner` and then the engine's reruns on a copy of `tests.flaky/<lang>` with
/// `seed`; `None` (reported as skipped) when the runner's tool is not installed.
fn reruns(runner: &str, lang: &str, seed: u64) -> Option<TestReruns> {
    let dir = tempfile::tempdir().unwrap();
    copy_repo(&format!("tests.flaky/{lang}"), dir.path());
    let registry = registry();
    let runner_producer = producer(registry, FactKind::TestResults, runner, lang);
    if let Availability::MissingTool { name, .. } =
        runner_producer.availability(&Ctx::new(dir.path().to_path_buf()))
    {
        eprintln!("SKIPPED: {name} not installed; engine.flake did not rerun {runner}");
        return None;
    }
    let baseline = runner_producer
        .produce(
            &produce_ctx(dir.path(), seed, json!({}), vec![project(lang)]),
            &Scope::default(),
            &mut Vec::new(),
        )
        .unwrap();
    assert!(
        baseline.downcast_ref::<TestResults>().unwrap().is_green(),
        "{runner}: {:?}",
        baseline.to_json()
    );
    let ctx = produce_ctx(dir.path(), seed, json!({}), vec![baseline]);
    Some(engine_produce(&ctx, lang).unwrap())
}

/// Each test's outcomes across the reruns, by ID.
fn histories(reruns: &TestReruns) -> Vec<(String, Vec<TestOutcome>)> {
    let mut ids: Vec<&str> = reruns
        .runs
        .iter()
        .flat_map(|run| run.tests.iter().map(|test| test.id.as_str()))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter()
        .map(|id| {
            let outcomes = reruns
                .runs
                .iter()
                .filter_map(|run| run.tests.iter().find(|test| test.id == id))
                .map(|test| test.outcome)
                .collect();
            (id.to_owned(), outcomes)
        })
        .collect()
}

/// What the reruns of a fixture must show: the victim passes in both declared reruns and fails
/// in the third, after its polluter; the settling test alternates.
fn assert_reruns(runner: &str, lang: &str, victim: &str, settles: &str) {
    let Some(reruns) = reruns(runner, lang, SEED) else {
        return;
    };
    let histories = histories(&reruns);
    let of = |id: &str| {
        histories.iter().find(|(test, _)| test == id).map_or_else(
            || panic!("{runner}: no {id} in {histories:#?}"),
            |(_, outcomes)| outcomes.clone(),
        )
    };
    assert_eq!(
        (of(victim), of(settles)),
        (vec![Passed, Passed, Failed], vec![Failed, Passed, Failed]),
        "{runner}: {:#?}",
        reruns.runs
    );
    assert_eq!(reruns.runner.as_str(), runner);
    assert_eq!(reruns.seed, SEED);
    assert_eq!(reruns.runs[0].order_key(), reruns.runs[1].order_key());
    assert_ne!(reruns.runs[0].order_key(), reruns.runs[2].order_key());
}

#[test]
fn pytest_reruns_reverse_the_declared_order() {
    assert_reruns(
        "tool.pytest",
        "python",
        "tests/test_ledger.py::LedgerTest::test_balance_starts_at_zero",
        "tests/test_settle.py::SettleTest::test_settles_on_every_other_run",
    );
}

#[test]
fn unittest_reruns_reverse_the_declared_order() {
    assert_reruns(
        "tool.unittest",
        "python",
        "tests.test_ledger.LedgerTest.test_balance_starts_at_zero",
        "tests.test_settle.SettleTest.test_settles_on_every_other_run",
    );
}

#[test]
fn go_test_reruns_shuffle_with_the_seed() {
    assert_reruns(
        "tool.go-test",
        "go",
        "example.com/ledger/TestBalanceStartsAtZero",
        "example.com/ledger/TestSettlesOnEveryOtherRun",
    );
}

#[test]
fn cargo_test_reruns_shuffle_with_the_seed() {
    assert_reruns(
        "tool.cargo-test",
        "rust",
        "ledger/src/lib.rs::tests::balance_starts_at_zero",
        "settle/tests/settle.rs::settles_on_every_other_run",
    );
}

/// Jest keeps a cache per project path in the temporary directory, so the test runs in one of its
/// own.
#[test]
fn jest_reruns_randomize_with_the_seed() {
    if ran_in_own_tmpdir("jest_reruns_randomize_with_the_seed") {
        return;
    }
    assert_reruns(
        "tool.jest",
        "javascript",
        "ledger.test.js > balance starts at zero",
        "settle.test.js > settles on every other run",
    );
}

#[test]
fn vitest_reruns_shuffle_with_the_seed() {
    assert_reruns(
        "tool.vitest",
        "typescript",
        "src/ledger.test.ts > balance starts at zero",
        "src/settle.test.ts > settles on every other run",
    );
}

#[test]
fn the_same_seed_repeats_the_reruns_orders() {
    let Some(first) = reruns("tool.go-test", "go", SEED) else {
        return;
    };
    let second = reruns("tool.go-test", "go", SEED).unwrap();
    let orders = |reruns: &TestReruns| -> Vec<Option<Vec<String>>> {
        reruns.runs.iter().map(|run| run.executed.clone()).collect()
    };
    assert_eq!(orders(&first), orders(&second));
}
