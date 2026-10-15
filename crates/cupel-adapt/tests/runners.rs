// ABOUTME: The runner producers as the registry holds them: table order, detection, pins, and real runs of
// ABOUTME: each runner on the test-results fixtures, skipped with a reason when the tool is not installed.
mod support;

use cupel_core::{
    Availability, Cancellation, Ctx, Error, FactKind, LangId, ProducerRegistry, Scope, TestOutcome,
    TestResults,
};
use cupel_fixtures::copy_repo;
use support::{lang, produce_ctx, registry, runner};

use TestOutcome::{Passed, Skipped};

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

/// Each test's ID and outcome.
fn outcomes(results: &TestResults) -> Vec<(String, TestOutcome)> {
    results
        .tests
        .iter()
        .map(|test| (test.id.clone(), test.outcome))
        .collect()
}

/// What a runner must report on its fixture: `tests` from the green suite, then the red
/// overlay's failing test `red`, with `line` among the lines of its failure output.
struct Expected {
    runner: &'static str,
    lang: &'static str,
    repo: &'static str,
    tests: &'static [(&'static str, TestOutcome)],
    red: &'static str,
    line: &'static str,
}

/// Runs `expected.runner` green and red and checks what it reported against `expected`.
fn assert_runs(expected: &Expected) {
    let Some(green) = run(expected.runner, expected.lang, expected.repo, false) else {
        return;
    };
    let tests: Vec<(String, TestOutcome)> = expected
        .tests
        .iter()
        .map(|(test, outcome)| ((*test).to_owned(), *outcome))
        .collect();
    assert_eq!(
        (green.is_green(), green.runner.as_str(), outcomes(&green)),
        (true, expected.runner, tests),
        "{} green",
        expected.runner
    );
    let red_run = run(expected.runner, expected.lang, expected.repo, true).unwrap();
    let failing: Vec<(&str, Option<&str>)> = red_run
        .failures()
        .map(|test| {
            let found = test
                .output
                .as_deref()
                .and_then(|output| output.lines().find(|seen| *seen == expected.line));
            (test.id.as_str(), found)
        })
        .collect();
    assert_eq!(
        (red_run.is_green(), failing),
        (false, vec![(expected.red, Some(expected.line))]),
        "{} red",
        expected.runner
    );
}

const CARGO_TEST: Expected = Expected {
    runner: "tool.cargo-test",
    lang: "rust",
    repo: "rust",
    tests: &[
        ("tally/src/lib.rs::tests::totals_a_huge_list", Skipped),
        ("tally/src/lib.rs::tests::totals_a_list", Passed),
        ("totals/tests/totals.rs::an_empty_list_totals_zero", Passed),
    ],
    red: "red/tests/red.rs::reports_a_wrong_total",
    line: "totalling 2 and 2",
};

const CARGO_NEXTEST: Expected = Expected {
    runner: "tool.cargo-nextest",
    lang: "rust",
    repo: "rust",
    tests: &[
        ("tally tests::totals_a_list", Passed),
        ("tally::totals an_empty_list_totals_zero", Passed),
    ],
    red: "tally::red reports_a_wrong_total",
    line: "totalling 2 and 2",
};

const GO_TEST: Expected = Expected {
    runner: "tool.go-test",
    lang: "go",
    repo: "go",
    tests: &[
        ("example.com/tally/TestTotalsAHugeList", Skipped),
        ("example.com/tally/TestTotalsAList", Passed),
    ],
    red: "example.com/tally/TestReportsAWrongTotal",
    line: "    red_test.go:7: totalling 2 and 2",
};

const PYTEST: Expected = Expected {
    runner: "tool.pytest",
    lang: "python",
    repo: "python",
    tests: &[
        (
            "tests/test_tally.py::TotalTest::test_totals_a_huge_list",
            Skipped,
        ),
        ("tests/test_tally.py::TotalTest::test_totals_a_list", Passed),
    ],
    red: "tests/test_red.py::RedTest::test_reports_a_wrong_total",
    line: "totalling 2 and 2",
};

const UNITTEST: Expected = Expected {
    runner: "tool.unittest",
    lang: "python",
    repo: "python",
    tests: &[
        (
            "tests.test_tally.TotalTest.test_totals_a_huge_list",
            Skipped,
        ),
        ("tests.test_tally.TotalTest.test_totals_a_list", Passed),
    ],
    red: "tests.test_red.RedTest.test_reports_a_wrong_total",
    line: "totalling 2 and 2",
};

const JEST: Expected = Expected {
    runner: "tool.jest",
    lang: "javascript",
    repo: "javascript",
    tests: &[
        ("tally.test.js > totals a huge list", Skipped),
        ("tally.test.js > totals a list", Passed),
    ],
    red: "red.test.js > reports a wrong total",
    line: "Error: expect(received).toBe(expected) // Object.is equality",
};

const VITEST: Expected = Expected {
    runner: "tool.vitest",
    lang: "typescript",
    repo: "typescript",
    tests: &[
        ("src/tally.test.ts > total > totals a huge list", Skipped),
        ("src/tally.test.ts > total > totals a list", Passed),
    ],
    red: "src/red.test.ts > reports a wrong total",
    line: "AssertionError: expected 4 to be 5 // Object.is equality",
};

#[test]
fn cargo_test_runs_the_rust_fixture() {
    assert_runs(&CARGO_TEST);
}

#[test]
fn cargo_nextest_runs_the_rust_fixture() {
    assert_runs(&CARGO_NEXTEST);
}

#[test]
fn go_test_runs_the_go_fixture() {
    assert_runs(&GO_TEST);
}

#[test]
fn pytest_runs_the_python_fixture() {
    assert_runs(&PYTEST);
}

#[test]
fn unittest_runs_the_python_fixture() {
    assert_runs(&UNITTEST);
}

#[test]
fn jest_runs_the_javascript_fixture() {
    assert_runs(&JEST);
}

#[test]
fn vitest_runs_the_typescript_fixture() {
    assert_runs(&VITEST);
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
