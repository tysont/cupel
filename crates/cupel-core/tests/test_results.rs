// ABOUTME: Tests for the `TestResults` payload (ADR-075): when a suite is green, which tests and runs make
// ABOUTME: it red, how output tails are cut, and what the suite hash covers.
use std::fs;

use cupel_core::{
    FactKind, OUTPUT_TAIL_LINES, ProducerId, RunnerInvocation, TestOutcome, TestResult,
    TestResults, output_tail, schema,
};

fn test(id: &str, file: Option<&str>, outcome: TestOutcome) -> TestResult {
    TestResult {
        id: id.to_owned(),
        file: file.map(str::to_owned),
        name: id.to_owned(),
        outcome,
        duration_ms: Some(1),
        output: None,
    }
}

fn invocation(completed: bool) -> RunnerInvocation {
    RunnerInvocation {
        dir: ".".to_owned(),
        command: vec!["pytest".to_owned()],
        exit_code: Some(if completed { 0 } else { 2 }),
        completed,
        output: None,
    }
}

fn results(tests: Vec<TestResult>, completed: bool) -> TestResults {
    let dir = tempfile::tempdir().unwrap();
    TestResults::new(
        ProducerId::from("tool.pytest"),
        vec![invocation(completed)],
        tests,
        dir.path(),
    )
}

fn failing_ids(results: &TestResults) -> Vec<&str> {
    results.failures().map(|test| test.id.as_str()).collect()
}

#[test]
fn passed_and_skipped_tests_of_a_completed_run_are_green() {
    let green = results(
        vec![
            test("b", None, TestOutcome::Passed),
            test("a", None, TestOutcome::Skipped),
        ],
        true,
    );
    assert_eq!(
        (green.is_green(), failing_ids(&green)),
        (true, Vec::<&str>::new())
    );
}

#[test]
fn failed_and_errored_tests_make_the_suite_red() {
    let red = results(
        vec![
            test("c", None, TestOutcome::Errored),
            test("a", None, TestOutcome::Passed),
            test("b", None, TestOutcome::Failed),
        ],
        true,
    );
    assert_eq!((red.is_green(), failing_ids(&red)), (false, vec!["b", "c"]));
}

#[test]
fn a_run_that_did_not_complete_is_red_with_no_failing_test() {
    let broken = results(vec![], false);
    let incomplete: Vec<Option<i32>> = broken
        .incomplete()
        .map(|invocation| invocation.exit_code)
        .collect();
    assert_eq!((broken.is_green(), incomplete), (false, vec![Some(2)]));
}

#[test]
fn tests_are_kept_in_id_order() {
    let sorted = results(
        vec![
            test("z", None, TestOutcome::Passed),
            test("m", None, TestOutcome::Passed),
            test("a", None, TestOutcome::Passed),
        ],
        true,
    );
    let ids: Vec<&str> = sorted.tests.iter().map(|test| test.id.as_str()).collect();
    assert_eq!(ids, ["a", "m", "z"]);
}

/// The suite hash of one passing test `id` in `file` holding `content`, in a fresh directory.
fn hash_of(id: &str, file: &str, content: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(file), content).unwrap();
    TestResults::new(
        ProducerId::from("tool.pytest"),
        vec![invocation(true)],
        vec![test(id, Some(file), TestOutcome::Passed)],
        dir.path(),
    )
    .suite_hash
}

#[test]
fn the_suite_hash_follows_test_ids_and_test_file_content() {
    let base = hash_of("t", "test_a.py", "assert 1");
    let hashes = [
        hash_of("t", "test_a.py", "assert 1"),
        hash_of("u", "test_a.py", "assert 1"),
        hash_of("t", "test_a.py", "assert 2"),
    ];
    let same: Vec<bool> = hashes.iter().map(|hash| *hash == base).collect();
    assert_eq!(same, [true, false, false]);
    assert_eq!(base.len(), 64);
}

#[test]
fn an_output_tail_keeps_the_last_lines_without_surrounding_blank_lines() {
    let text = (1..=OUTPUT_TAIL_LINES + 5)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let tail = output_tail(&format!("\n\n{text}\n\n")).unwrap();
    let lines: Vec<&str> = tail.lines().collect();
    assert_eq!(
        (lines.len(), lines[0], lines[lines.len() - 1]),
        (OUTPUT_TAIL_LINES, "line 6", "line 45")
    );
    assert_eq!(output_tail(" \n \n"), None);
}

#[test]
fn outcomes_serialize_in_kebab_case_and_the_fact_is_version_2() {
    let names: Vec<String> = [
        TestOutcome::Passed,
        TestOutcome::Failed,
        TestOutcome::Skipped,
        TestOutcome::Errored,
    ]
    .map(|outcome| {
        serde_json::to_value(outcome)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned()
    })
    .to_vec();
    assert_eq!(names, ["passed", "failed", "skipped", "errored"]);
    assert_eq!(schema::fact_version(FactKind::TestResults), Some(2));
}
