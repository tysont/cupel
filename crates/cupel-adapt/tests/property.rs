// ABOUTME: The property engine (ADR-095): what it does without a baseline or a property, which runners take which
// ABOUTME: framework's budget, and real budgeted runs of each framework on the gen.property fixtures (skipped when absent).
use std::fs;
use std::path::Path;

use cupel_core::syntax::{ProjectInfo, ProjectRoot, Span, TestCase, TestFile, TestFrames};
use cupel_core::{
    Availability, Cancellation, Ctx, Error, FactKind, LangId, ProduceCtx, Produced, ProducerId,
    ProducerRegistry, PropertyOutcome, PropertyResults, RegisteredProducer, Scope, TestResults,
    Writer,
};
use cupel_fixtures::copy_repo;
use cupel_fixtures::matrix::Registries;

/// The seed the matrix goldens run with.
const SEED: u64 = cupel_fixtures::matrix::SEED;

fn registry() -> &'static ProducerRegistry {
    &Registries::shared().producers
}

fn producer(kind: FactKind, id: &str, lang: &str) -> &'static dyn RegisteredProducer {
    registry()
        .producers_for(kind, &Registries::shared().lang(lang))
        .find(|producer| id.is_empty() || producer.id().as_str() == id)
        .unwrap_or_else(|| panic!("{id} produces {kind} for {lang}"))
}

fn project(lang: &str) -> Produced {
    Produced::new(ProjectInfo {
        roots: vec![ProjectRoot {
            path: ".".to_owned(),
            lang: LangId::from(lang),
            build_files: Vec::new(),
            members: Vec::new(),
            packages: Vec::new(),
            skip_dirs: Vec::new(),
        }],
    })
}

fn produce_ctx(root: &Path, cases: Option<u64>, upstream: Vec<Produced>) -> ProduceCtx {
    let mut ctx = ProduceCtx::new(
        Ctx::new(root.to_path_buf()),
        Writer::new(root, false).unwrap().confined(),
        Cancellation::new(),
    );
    ctx.seed = SEED;
    ctx.focus.cases = cases;
    ctx.runners = registry().runners();
    for fact in upstream {
        ctx.upstream.insert(fact.share());
    }
    ctx
}

fn engine(ctx: &ProduceCtx, lang: &str, scope: &Scope) -> cupel_core::Result<PropertyResults> {
    producer(FactKind::PropertyResults, "engine.property", lang)
        .produce(ctx, scope, &mut Vec::new())
        .map(|fact| fact.downcast::<PropertyResults>().unwrap())
}

/// A green baseline of `runner` that ran no test.
fn green(runner: &str) -> Produced {
    Produced::new(TestResults {
        runner: ProducerId::from(runner),
        invocations: Vec::new(),
        tests: Vec::new(),
        suite_hash: String::new(),
    })
}

fn frames(file: &str, property: Option<&str>) -> Produced {
    let span = Span {
        line: 1,
        col: 1,
        end_line: 2,
        end_col: 1,
    };
    Produced::new(TestFrames {
        files: vec![TestFile {
            file: file.to_owned(),
            framework: "pytest".to_owned(),
            setup: Vec::new(),
            cases: vec![TestCase {
                name: "test_it".to_owned(),
                span,
                assertions: Vec::new(),
                mocks: Vec::new(),
                skip: None,
                property: property.map(str::to_owned),
            }],
        }],
    })
}

#[test]
fn the_engine_serves_the_languages_whose_frameworks_it_budgets() {
    let langs: Vec<&str> = ["go", "javascript", "python", "rust", "typescript"]
        .into_iter()
        .filter(|lang| {
            registry()
                .producers_for(FactKind::PropertyResults, &Registries::shared().lang(lang))
                .any(|producer| producer.id().as_str() == "engine.property")
        })
        .collect();
    assert_eq!(langs, ["javascript", "python", "rust", "typescript"]);
    let engine = producer(FactKind::PropertyResults, "engine.property", "rust");
    assert_eq!(
        engine.needs(),
        [FactKind::TestResults, FactKind::TestFrames]
    );
    assert_eq!(
        engine.availability(&Ctx::new(".".into())),
        Availability::Ready
    );
}

#[test]
fn without_a_property_in_scope_the_engine_runs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = produce_ctx(
        dir.path(),
        None,
        vec![
            green("tool.pytest"),
            frames("tests/test_a.py", None),
            project("python"),
        ],
    );
    let results = engine(&ctx, "python", &Scope::default()).unwrap();
    assert_eq!(results.runner.as_str(), "tool.pytest");
    assert_eq!(results.seed, SEED);
    assert_eq!(results.cases, 1_000);
    assert_eq!(results.frameworks, ["hypothesis"]);
    assert!(results.properties.is_empty());
    assert!(!dir.path().join(".cupel").exists(), "nothing written");
}

#[test]
fn a_framework_the_runner_cannot_budget_is_an_error_naming_the_runners_that_can() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = produce_ctx(
        dir.path(),
        None,
        vec![
            green("tool.unittest"),
            frames("tests/test_a.py", Some("hypothesis")),
            project("python"),
        ],
    );
    let error = engine(&ctx, "python", &Scope::default()).unwrap_err();
    assert_eq!(
        error,
        Error::InvalidValue {
            what: "runner for hypothesis properties",
            value: "tool.unittest".to_owned(),
            expected: "tool.pytest",
        }
    );
}

/// Every file under `root`, relative to it, except what the runs build into (`target`,
/// `.cupel`).
fn files(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if matches!(name.as_str(), "target" | ".cupel" | ".hypothesis") {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else {
                found.push(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    found.sort();
    found
}

/// The baseline of `runner`, the language's `TestFrames`, and the engine's run on a copy of
/// `gen.property/<lang>`, which must write nothing outside `.cupel/`; `None` (reported as
/// skipped) when the runner's tool is not installed.
fn budgeted(runner: &str, lang: &str, cases: Option<u64>) -> Option<PropertyResults> {
    let dir = tempfile::tempdir().unwrap();
    copy_repo(&format!("gen.property/{lang}"), dir.path());
    let sources = files(dir.path());
    let runner_producer = producer(FactKind::TestResults, runner, lang);
    if let Availability::MissingTool { name, .. } =
        runner_producer.availability(&Ctx::new(dir.path().to_path_buf()))
    {
        eprintln!("SKIPPED: {name} not installed; engine.property did not run {runner}");
        return None;
    }
    let baseline = runner_producer
        .produce(
            &produce_ctx(dir.path(), None, vec![project(lang)]),
            &Scope::default(),
            &mut Vec::new(),
        )
        .unwrap();
    assert!(
        baseline.downcast_ref::<TestResults>().unwrap().is_green(),
        "{runner}: {:?}",
        baseline.to_json()
    );
    let scope = Scope::of_files(sources);
    let frames = producer(FactKind::TestFrames, "", lang)
        .produce(
            &produce_ctx(dir.path(), None, Vec::new()),
            &scope,
            &mut Vec::new(),
        )
        .unwrap();
    let ctx = produce_ctx(dir.path(), cases, vec![baseline, frames, project(lang)]);
    let before = files(dir.path());
    let results = engine(&ctx, lang, &scope).unwrap();
    assert_eq!(files(dir.path()), before, "{runner} wrote into the tree");
    Some(results)
}

/// Each property's name, framework, outcome, counterexample, and whether it took the seed.
fn rows(results: &PropertyResults) -> Vec<(String, String, PropertyOutcome, Option<String>, bool)> {
    results
        .properties
        .iter()
        .map(|property| {
            (
                property.name.clone(),
                property.framework.clone(),
                property.outcome,
                property.counterexample.clone(),
                property.seeded,
            )
        })
        .collect()
}

fn row(
    name: &str,
    framework: &str,
    outcome: PropertyOutcome,
    counterexample: Option<&str>,
    seeded: bool,
) -> (String, String, PropertyOutcome, Option<String>, bool) {
    (
        name.to_owned(),
        framework.to_owned(),
        outcome,
        counterexample.map(str::to_owned),
        seeded,
    )
}

#[test]
fn proptest_and_quickcheck_run_with_the_budget_under_cargo_test() {
    let Some(results) = budgeted("tool.cargo-test", "rust", None) else {
        return;
    };
    assert_eq!(
        rows(&results),
        [
            row(
                "digits_counts_decimal_places",
                "proptest",
                PropertyOutcome::Falsified,
                Some("n = 100000"),
                true
            ),
            row(
                "digit_counts_stay_within_ten",
                "quickcheck",
                PropertyOutcome::Passed,
                None,
                false
            ),
        ]
    );
    assert_eq!(results.frameworks, ["proptest", "quickcheck"]);
    assert_eq!(
        results.properties[0].test.as_deref(),
        Some("digits/tests/digits.rs::digits_counts_decimal_places")
    );
}

#[test]
fn hypothesis_runs_with_the_budget_under_pytest() {
    let Some(results) = budgeted("tool.pytest", "python", None) else {
        return;
    };
    assert_eq!(
        rows(&results),
        [row(
            "test_digits_counts_decimal_places",
            "hypothesis",
            PropertyOutcome::Falsified,
            Some("n=100000"),
            true
        )]
    );
}

#[test]
fn fast_check_runs_with_the_budget_under_jest_and_vitest() {
    for (runner, lang) in [("tool.jest", "javascript"), ("tool.vitest", "typescript")] {
        let Some(results) = budgeted(runner, lang, None) else {
            continue;
        };
        assert_eq!(
            rows(&results),
            [row(
                "digits counts decimal places",
                "fast-check",
                PropertyOutcome::Falsified,
                Some("[100000]"),
                true
            )],
            "{runner}"
        );
    }
}

#[test]
fn a_small_case_budget_misses_the_counterexample() {
    let Some(results) = budgeted("tool.pytest", "python", Some(5)) else {
        return;
    };
    assert_eq!(results.cases, 5);
    assert_eq!(results.properties[0].outcome, PropertyOutcome::Passed);
}
