// ABOUTME: What the flake engine reads from its reruns, against real output committed under
// ABOUTME: `cupel-fixtures/captures/flake/`: each runner's shuffled or reversed rerun of the tests.flaky fixture.

use cupel_adapt::flake::go_test_order;
use cupel_adapt::report::{jest_json, junit, libtest, test2json, unittest};
use cupel_core::{TestOutcome, TestResult};
use cupel_fixtures::Capture;

use TestOutcome::{Failed, Passed};

/// Each test's ID and outcome, in the order the parser returned them.
fn ran(tests: &[TestResult]) -> Vec<(String, TestOutcome)> {
    tests
        .iter()
        .map(|test| (test.id.clone(), test.outcome))
        .collect()
}

fn row(id: &str, outcome: TestOutcome) -> (String, TestOutcome) {
    (id.to_owned(), outcome)
}

#[test]
fn a_shuffled_go_rerun_reports_the_order_its_tests_started_in() {
    let stdout = Capture::of("flake/go-test", "shuffled").file("stdout.jsonl");
    assert_eq!(
        go_test_order(&stdout),
        [
            "example.com/ledger/TestDepositChangesBalance",
            "example.com/ledger/TestBalanceStartsAtZero",
            "example.com/ledger/TestSettlesOnEveryOtherRun",
        ]
    );
    let tests = test2json::parse(&stdout, "");
    let victim = tests
        .iter()
        .find(|test| test.id == "example.com/ledger/TestBalanceStartsAtZero")
        .unwrap();
    assert_eq!(victim.outcome, Failed);
}

/// A rerun's tests parsed from a capture, and what they must be, in the order the parser
/// returned them.
type Reading = (
    &'static str,
    Vec<TestResult>,
    [(&'static str, TestOutcome); 3],
);

#[test]
fn a_serial_or_reversed_rerun_lists_its_tests_in_the_order_they_ran() {
    let readings: [Reading; 3] = [
        (
            "cargo test, shuffled",
            libtest::parse(&Capture::of("flake/cargo-test", "shuffled").file("output.txt")),
            [
                ("ledger/src/lib.rs::tests::deposit_changes_balance", Passed),
                ("ledger/src/lib.rs::tests::balance_starts_at_zero", Failed),
                ("settle/tests/settle.rs::settles_on_every_other_run", Failed),
            ],
        ),
        (
            "pytest, reversed",
            junit::pytest(&Capture::of("flake/pytest", "reversed").file("junit.xml")).unwrap(),
            [
                (
                    "tests/test_settle.py::SettleTest::test_settles_on_every_other_run",
                    Passed,
                ),
                (
                    "tests/test_ledger.py::LedgerTest::test_deposit_changes_balance",
                    Passed,
                ),
                (
                    "tests/test_ledger.py::LedgerTest::test_balance_starts_at_zero",
                    Failed,
                ),
            ],
        ),
        (
            "unittest, reversed",
            unittest::parse(&Capture::of("flake/unittest", "reversed").file("output.txt")),
            [
                (
                    "tests.test_settle.SettleTest.test_settles_on_every_other_run",
                    Passed,
                ),
                (
                    "tests.test_ledger.LedgerTest.test_deposit_changes_balance",
                    Passed,
                ),
                (
                    "tests.test_ledger.LedgerTest.test_balance_starts_at_zero",
                    Failed,
                ),
            ],
        ),
    ];
    for (case, tests, expected) in readings {
        let expected: Vec<(String, TestOutcome)> = expected
            .iter()
            .map(|(id, outcome)| row(id, *outcome))
            .collect();
        assert_eq!(ran(&tests), expected, "{case}");
    }
}

/// The outcomes of a Jest JSON report captured as `runner`'s `case`, sorted by ID.
fn jest_outcomes(runner: &str, case: &str) -> Vec<(String, TestOutcome)> {
    let capture = Capture::of(runner, case);
    let mut tests = jest_json::parse(&capture.file("report.json"), &capture.ran_in).unwrap();
    tests.sort_by(|a, b| a.id.cmp(&b.id));
    ran(&tests)
}

#[test]
fn a_shuffled_jest_or_vitest_rerun_fails_the_victim_after_its_polluter() {
    let jest = jest_outcomes("flake/jest", "randomized");
    let vitest = jest_outcomes("flake/vitest", "shuffled");
    assert_eq!(
        (jest, vitest),
        (
            vec![
                row("ledger.test.js > balance starts at zero", Failed),
                row("ledger.test.js > deposit changes balance", Passed),
                row("settle.test.js > settles on every other run", Passed),
            ],
            vec![
                row("src/ledger.test.ts > balance starts at zero", Failed),
                row("src/ledger.test.ts > deposit changes balance", Passed),
                row("src/settle.test.ts > settles on every other run", Passed),
            ]
        )
    );
}
