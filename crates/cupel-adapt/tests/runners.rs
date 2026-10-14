// ABOUTME: The runner producers as the registry holds them: table order, detection, pins, and real runs of
// ABOUTME: each runner on the test-results fixtures, skipped with a reason when the tool is not installed.
use std::path::Path;

use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{
    Availability, Cancellation, Ctx, Error, FactKind, Lang, LangFamily, LangId, ProduceCtx,
    Produced, ProducerRegistry, RegisteredProducer, Scope, TestOutcome, TestResults, Writer,
};
use cupel_fixtures::copy_repo;

use TestOutcome::{Passed, Skipped};

fn registry() -> ProducerRegistry {
    let mut registry = ProducerRegistry::default();
    cupel_adapt::register(&mut registry);
    registry
}

fn lang(id: &str) -> Lang {
    let family = if matches!(id, "javascript" | "typescript") {
        "ecma"
    } else {
        id
    };
    Lang {
        id: LangId::from(id),
        family: LangFamily::from(family),
    }
}

/// The `TestResults` producers for `lang` at `root` in preference order, under `ctx`'s pins.
fn preferred(registry: &ProducerRegistry, lang: &str, ctx: &Ctx) -> Vec<String> {
    registry
        .candidates(FactKind::TestResults, &self::lang(lang), ctx)
        .unwrap()
        .iter()
        .map(|producer| producer.id().to_string())
        .collect()
}

#[test]
fn runners_register_for_their_languages_in_toolchain_table_order() {
    let registry = registry();
    let ctx = Ctx::new(std::env::temp_dir().join("cupel-no-such-project"));
    let ids = ["rust", "go", "python", "javascript", "typescript"]
        .map(|lang| preferred(&registry, lang, &ctx));
    assert_eq!(
        ids,
        [
            vec!["tool.cargo-test", "tool.cargo-nextest"],
            vec!["tool.go-test"],
            vec!["tool.unittest", "tool.pytest"],
            vec!["tool.vitest", "tool.jest"],
            vec!["tool.vitest", "tool.jest"],
        ]
        .map(|ids| ids.into_iter().map(str::to_owned).collect::<Vec<_>>())
    );
}

#[test]
fn detection_puts_the_runner_a_fixture_uses_first() {
    let registry = registry();
    let at = |repo: &str, lang: &str| {
        preferred(
            &registry,
            lang,
            &Ctx::new(cupel_fixtures::repo_path(&format!("test-results/{repo}"))),
        )
    };
    assert_eq!(
        [
            at("python", "python"),
            at("javascript", "javascript"),
            at("typescript", "typescript")
        ],
        [
            ["tool.pytest", "tool.unittest"],
            ["tool.jest", "tool.vitest"],
            ["tool.vitest", "tool.jest"],
        ]
        .map(|ids| ids.map(str::to_owned).to_vec())
    );
}

#[test]
fn a_pin_keeps_only_the_named_runner() {
    let pinned = Ctx {
        lang: [(
            LangId::from("python"),
            serde_json::json!({"runner": "unittest"})
                .as_object()
                .unwrap()
                .clone(),
        )]
        .into(),
        ..Ctx::new(cupel_fixtures::repo_path("test-results/python"))
    };
    assert_eq!(preferred(&registry(), "python", &pinned), ["tool.unittest"]);
}

/// A run of the runner `id` for `lang` on a copy of `test-results/<repo>`, with the red overlay
/// when `red`; `None` (reported as skipped) when the runner's tool is not installed.
fn run(id: &str, lang: &str, repo: &str, red: bool) -> Option<TestResults> {
    let dir = tempfile::tempdir().unwrap();
    copy_repo(&format!("test-results/{repo}"), dir.path());
    if red {
        copy_repo(&format!("test-results/red/{repo}"), dir.path());
    }
    let registry = registry();
    let producer = runner(&registry, id, lang);
    let ctx = Ctx::new(dir.path().to_path_buf());
    if let Availability::MissingTool { name, .. } = producer.availability(&ctx) {
        eprintln!("SKIPPED: {name} not installed; {id} was not run");
        return None;
    }
    let fact = producer
        .produce(
            &produce_ctx(dir.path(), lang, Cancellation::new()),
            &Scope::default(),
            &mut Vec::new(),
        )
        .unwrap();
    Some(fact.downcast::<TestResults>().unwrap())
}

fn runner<'a>(registry: &'a ProducerRegistry, id: &str, lang: &str) -> &'a dyn RegisteredProducer {
    registry
        .producers_for(FactKind::TestResults, &self::lang(lang))
        .find(|producer| producer.id().as_str() == id)
        .unwrap()
}

/// The context a runner gets for the repository at `root`: one project root, `.`.
fn produce_ctx(root: &Path, lang: &str, cancel: Cancellation) -> ProduceCtx {
    let mut ctx = ProduceCtx::new(
        Ctx::new(root.to_path_buf()),
        Writer::new(root, false).unwrap().confined(),
        cancel,
    );
    ctx.upstream.insert(
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
        .share(),
    );
    ctx
}

/// Each test's ID and outcome.
fn outcomes(results: &TestResults) -> Vec<(String, TestOutcome)> {
    results
        .tests
        .iter()
        .map(|test| (test.id.clone(), test.outcome))
        .collect()
}

/// Runs `id` green and red and checks what it reported: `tests` green, then `red` failing too,
/// with `line` among the lines of its failure output.
fn assert_runs(
    (id, lang, repo): (&str, &str, &str),
    tests: &[(&str, TestOutcome)],
    red: &str,
    line: &str,
) {
    let Some(green) = run(id, lang, repo, false) else {
        return;
    };
    let expected: Vec<(String, TestOutcome)> = tests
        .iter()
        .map(|(test, outcome)| ((*test).to_owned(), *outcome))
        .collect();
    assert_eq!(
        (green.is_green(), green.runner.as_str(), outcomes(&green)),
        (true, id, expected),
        "{id} green"
    );
    let red_run = run(id, lang, repo, true).unwrap();
    let failing: Vec<(&str, Option<&str>)> = red_run
        .failures()
        .map(|test| {
            let found = test
                .output
                .as_deref()
                .and_then(|output| output.lines().find(|seen| *seen == line));
            (test.id.as_str(), found)
        })
        .collect();
    assert_eq!(
        (red_run.is_green(), failing),
        (false, vec![(red, Some(line))]),
        "{id} red"
    );
}

#[test]
fn cargo_test_runs_the_rust_fixture() {
    assert_runs(
        ("tool.cargo-test", "rust", "rust"),
        &[
            ("tally/src/lib.rs::tests::totals_a_huge_list", Skipped),
            ("tally/src/lib.rs::tests::totals_a_list", Passed),
            ("totals/tests/totals.rs::an_empty_list_totals_zero", Passed),
        ],
        "red/tests/red.rs::reports_a_wrong_total",
        "totalling 2 and 2",
    );
}

#[test]
fn cargo_nextest_runs_the_rust_fixture() {
    assert_runs(
        ("tool.cargo-nextest", "rust", "rust"),
        &[
            ("tally tests::totals_a_list", Passed),
            ("tally::totals an_empty_list_totals_zero", Passed),
        ],
        "tally::red reports_a_wrong_total",
        "totalling 2 and 2",
    );
}

#[test]
fn go_test_runs_the_go_fixture() {
    assert_runs(
        ("tool.go-test", "go", "go"),
        &[
            ("example.com/tally/TestTotalsAHugeList", Skipped),
            ("example.com/tally/TestTotalsAList", Passed),
        ],
        "example.com/tally/TestReportsAWrongTotal",
        "    red_test.go:7: totalling 2 and 2",
    );
}

#[test]
fn pytest_runs_the_python_fixture() {
    assert_runs(
        ("tool.pytest", "python", "python"),
        &[
            (
                "tests/test_tally.py::TotalTest::test_totals_a_huge_list",
                Skipped,
            ),
            ("tests/test_tally.py::TotalTest::test_totals_a_list", Passed),
        ],
        "tests/test_red.py::RedTest::test_reports_a_wrong_total",
        "totalling 2 and 2",
    );
}

#[test]
fn unittest_runs_the_python_fixture() {
    assert_runs(
        ("tool.unittest", "python", "python"),
        &[
            (
                "tests.test_tally.TotalTest.test_totals_a_huge_list",
                Skipped,
            ),
            ("tests.test_tally.TotalTest.test_totals_a_list", Passed),
        ],
        "tests.test_red.RedTest.test_reports_a_wrong_total",
        "totalling 2 and 2",
    );
}

#[test]
fn jest_runs_the_javascript_fixture() {
    assert_runs(
        ("tool.jest", "javascript", "javascript"),
        &[
            ("tally.test.js > totals a huge list", Skipped),
            ("tally.test.js > totals a list", Passed),
        ],
        "red.test.js > reports a wrong total",
        "Error: expect(received).toBe(expected) // Object.is equality",
    );
}

#[test]
fn vitest_runs_the_typescript_fixture() {
    assert_runs(
        ("tool.vitest", "typescript", "typescript"),
        &[
            ("src/tally.test.ts > total > totals a huge list", Skipped),
            ("src/tally.test.ts > total > totals a list", Passed),
        ],
        "src/red.test.ts > reports a wrong total",
        "AssertionError: expected 4 to be 5 // Object.is equality",
    );
}

#[test]
fn a_cancelled_run_stops_before_running_anything() {
    let dir = tempfile::tempdir().unwrap();
    let registry = registry();
    let cancel = Cancellation::new();
    cancel.cancel();
    let produced = runner(&registry, "tool.unittest", "python").produce(
        &produce_ctx(dir.path(), "python", cancel),
        &Scope::default(),
        &mut Vec::new(),
    );
    assert_eq!(produced.err(), Some(Error::Cancelled));
}
