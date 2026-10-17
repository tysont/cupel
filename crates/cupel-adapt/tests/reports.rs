// ABOUTME: The runner report parsers against real output committed under `cupel-fixtures/captures/`: each
// ABOUTME: runner's green and red runs of the test-results fixture, and a Go package that does not build.
use cupel_adapt::report::{jest_json, junit, libtest, test2json, unittest};
use cupel_core::{TestOutcome, TestResult};
use cupel_fixtures::Capture;

use TestOutcome::{Errored, Failed, Passed, Skipped};

/// Each test as `(id, file, outcome, duration_ms)`.
type Shape = (String, Option<String>, TestOutcome, Option<u64>);

fn shape(tests: &[TestResult]) -> Vec<Shape> {
    tests
        .iter()
        .map(|test| {
            (
                test.id.clone(),
                test.file.clone(),
                test.outcome,
                test.duration_ms,
            )
        })
        .collect()
}

fn row(id: &str, file: Option<&str>, outcome: TestOutcome, duration_ms: Option<u64>) -> Shape {
    (id.to_owned(), file.map(str::to_owned), outcome, duration_ms)
}

/// The first and last lines of each test's failure output, by ID, for tests that have one.
fn outputs(tests: &[TestResult]) -> Vec<(String, String, String)> {
    tests
        .iter()
        .filter_map(|test| {
            let output = test.output.as_deref()?;
            let first = output.lines().next().unwrap_or_default().to_owned();
            let last = output.lines().last().unwrap_or_default().to_owned();
            Some((test.id.clone(), first, last))
        })
        .collect()
}

fn output(id: &str, first: &str, last: &str) -> (String, String, String) {
    (id.to_owned(), first.to_owned(), last.to_owned())
}

#[test]
fn cargo_test_output_names_each_test_by_binary_and_file_and_keeps_failure_output() {
    let tests = libtest::parse(&Capture::of("cargo-test", "red").file("output.txt"));
    assert_eq!(
        shape(&tests),
        [
            row(
                "tally/src/lib.rs::tests::totals_a_huge_list",
                Some("src/lib.rs"),
                Skipped,
                None
            ),
            row(
                "tally/src/lib.rs::tests::totals_a_list",
                Some("src/lib.rs"),
                Passed,
                None
            ),
            row(
                "red/tests/red.rs::reports_a_wrong_total",
                Some("tests/red.rs"),
                Failed,
                None
            ),
            row(
                "totals/tests/totals.rs::an_empty_list_totals_zero",
                Some("tests/totals.rs"),
                Passed,
                None
            ),
        ]
    );
    assert_eq!(
        outputs(&tests),
        [output(
            "red/tests/red.rs::reports_a_wrong_total",
            "totalling 2 and 2",
            "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace"
        )]
    );
}

#[test]
fn a_green_cargo_test_run_has_no_failure() {
    let tests = libtest::parse(&Capture::of("cargo-test", "green").file("output.txt"));
    let outcomes: Vec<TestOutcome> = tests.iter().map(|test| test.outcome).collect();
    assert_eq!(outcomes, [Skipped, Passed, Passed]);
}

#[test]
fn nextest_junit_names_each_test_by_binary_id_with_its_time_and_failure_output() {
    let tests = junit::nextest(&Capture::of("cargo-nextest", "red").file("junit.xml")).unwrap();
    assert_eq!(
        shape(&tests),
        [
            row("tally tests::totals_a_list", None, Passed, Some(9)),
            row(
                "tally::totals an_empty_list_totals_zero",
                None,
                Passed,
                Some(9)
            ),
            row("tally::red reports_a_wrong_total", None, Failed, Some(9)),
        ]
    );
    assert_eq!(
        outputs(&tests),
        [output(
            "tally::red reports_a_wrong_total",
            "thread 'reports_a_wrong_total' (19120668) panicked at tests/red.rs:6:5:",
            "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace"
        )]
    );
}

#[test]
fn pytest_junit_names_each_test_by_node_id_and_file() {
    let tests = junit::pytest(&Capture::of("pytest", "red").file("junit.xml")).unwrap();
    assert_eq!(
        shape(&tests),
        [
            row(
                "tests/test_red.py::RedTest::test_reports_a_wrong_total",
                Some("tests/test_red.py"),
                Failed,
                Some(1)
            ),
            row(
                "tests/test_tally.py::TotalTest::test_totals_a_huge_list",
                Some("tests/test_tally.py"),
                Skipped,
                Some(1)
            ),
            row(
                "tests/test_tally.py::TotalTest::test_totals_a_list",
                Some("tests/test_tally.py"),
                Passed,
                Some(1)
            ),
        ]
    );
    assert_eq!(
        outputs(&tests),
        [output(
            "tests/test_red.py::RedTest::test_reports_a_wrong_total",
            "self = <tests.test_red.RedTest testMethod=test_reports_a_wrong_total>",
            "--------------------------------- Captured Err ---------------------------------"
        )]
    );
}

#[test]
fn a_document_that_is_not_junit_is_no_report() {
    assert_eq!(
        (
            junit::parse("<html><body/></html>"),
            junit::parse("not xml")
        ),
        (None, None)
    );
}

#[test]
fn unittest_output_names_each_test_by_its_dotted_id_and_derives_its_file() {
    let tests = unittest::parse(&Capture::of("unittest", "red").file("output.txt"));
    assert_eq!(
        shape(&tests),
        [
            row(
                "tests.test_red.RedTest.test_reports_a_wrong_total",
                Some("tests/test_red.py"),
                Failed,
                None
            ),
            row(
                "tests.test_tally.TotalTest.test_totals_a_huge_list",
                Some("tests/test_tally.py"),
                Skipped,
                None
            ),
            row(
                "tests.test_tally.TotalTest.test_totals_a_list",
                Some("tests/test_tally.py"),
                Passed,
                None
            ),
        ]
    );
    assert_eq!(
        outputs(&tests),
        [output(
            "tests.test_red.RedTest.test_reports_a_wrong_total",
            "Traceback (most recent call last):",
            "totalling 2 and 2"
        )]
    );
}

#[test]
fn go_test_events_name_each_test_by_package_and_keep_its_log() {
    let capture = Capture::of("go-test", "red");
    let tests = test2json::parse(&capture.file("stdout.jsonl"), "");
    assert_eq!(
        shape(&tests),
        [
            row(
                "example.com/tally/TestReportsAWrongTotal",
                None,
                Failed,
                Some(0)
            ),
            row(
                "example.com/tally/TestTotalsAHugeList",
                None,
                Skipped,
                Some(0)
            ),
            row("example.com/tally/TestTotalsAList", None, Passed, Some(0)),
        ]
    );
    assert_eq!(
        outputs(&tests),
        [output(
            "example.com/tally/TestReportsAWrongTotal",
            "=== RUN   TestReportsAWrongTotal",
            "--- FAIL: TestReportsAWrongTotal (0.00s)"
        )]
    );
}

#[test]
fn a_go_package_that_does_not_build_is_one_errored_test_with_the_compiler_output() {
    let capture = Capture::of("go-test", "broken");
    let tests = test2json::parse(&capture.file("stdout.jsonl"), &capture.file("stderr.txt"));
    assert_eq!(
        shape(&tests),
        [row("example.com/tally", None, Errored, None)]
    );
    assert_eq!(
        outputs(&tests),
        [output(
            "example.com/tally",
            "FAIL\texample.com/tally [build failed]",
            "./broken.go:3:28: cannot use \"not a number\" (untyped string constant) as int value \
             in return statement"
        )]
    );
}

#[test]
fn jest_json_names_each_test_by_file_and_titles_relative_to_the_run() {
    let capture = Capture::of("jest", "red");
    let tests = jest_json::parse(&capture.file("report.json"), &capture.ran_in).unwrap();
    assert_eq!(
        shape(&tests),
        [
            row(
                "tally.test.js > totals a list",
                Some("tally.test.js"),
                Passed,
                Some(2)
            ),
            row(
                "tally.test.js > totals a huge list",
                Some("tally.test.js"),
                Skipped,
                None
            ),
            row(
                "red.test.js > reports a wrong total",
                Some("red.test.js"),
                Failed,
                Some(4)
            ),
        ]
    );
    let first_lines: Vec<(String, String)> = outputs(&tests)
        .into_iter()
        .map(|(id, first, _)| (id, first))
        .collect();
    assert_eq!(
        first_lines,
        [(
            "red.test.js > reports a wrong total".to_owned(),
            "Error: expect(received).toBe(expected) // Object.is equality".to_owned()
        )]
    );
}

#[test]
fn jest_failure_messages_keep_their_lines_and_drop_stack_frames() {
    let capture = Capture::of("property/jest", "falsified");
    let tests = jest_json::parse(&capture.file("report.json"), &capture.ran_in).unwrap();
    let output = tests
        .iter()
        .find(|test| test.outcome == Failed)
        .and_then(|test| test.output.as_deref())
        .unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(
        lines[..4],
        [
            "Error: Property failed after 8 tests",
            "{ seed: 3, path: \"7:4:1:0:0:1:4:0:0\", endOnFailure: true }",
            "Counterexample: [100000]",
            "Shrunk 8 time(s)",
        ]
    );
    assert!(
        lines
            .iter()
            .all(|line| !line.trim_start().starts_with("at ")),
        "{output}"
    );
    assert!(output.contains("[cause]: Error: expect(received).toBe(expected)"));
}

#[test]
fn vitest_json_is_read_like_jest_with_describe_blocks_in_the_id() {
    let capture = Capture::of("vitest", "red");
    let tests = jest_json::parse(&capture.file("report.json"), &capture.ran_in).unwrap();
    assert_eq!(
        shape(&tests),
        [
            row(
                "src/red.test.ts > reports a wrong total",
                Some("src/red.test.ts"),
                Failed,
                Some(4)
            ),
            row(
                "src/tally.test.ts > total > totals a list",
                Some("src/tally.test.ts"),
                Passed,
                Some(1)
            ),
            row(
                "src/tally.test.ts > total > totals a huge list",
                Some("src/tally.test.ts"),
                Skipped,
                None
            ),
        ]
    );
}

#[test]
fn a_green_run_of_every_runner_has_no_failing_test() {
    let failing = |tests: Vec<TestResult>| {
        tests
            .iter()
            .filter(|test| test.outcome.is_failure())
            .count()
    };
    let jest = Capture::of("jest", "green");
    let vitest = Capture::of("vitest", "green");
    let counts = [
        failing(junit::nextest(&Capture::of("cargo-nextest", "green").file("junit.xml")).unwrap()),
        failing(junit::pytest(&Capture::of("pytest", "green").file("junit.xml")).unwrap()),
        failing(unittest::parse(
            &Capture::of("unittest", "green").file("output.txt"),
        )),
        failing(test2json::parse(
            &Capture::of("go-test", "green").file("stdout.jsonl"),
            "",
        )),
        failing(jest_json::parse(&jest.file("report.json"), &jest.ran_in).unwrap()),
        failing(jest_json::parse(&vitest.file("report.json"), &vitest.ran_in).unwrap()),
    ];
    assert_eq!(counts, [0; 6]);
}
