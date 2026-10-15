// ABOUTME: tests.flaky over hand-built TestReruns and TestFrames, with no engine (ADR-078): flaky versus
// ABOUTME: order-dependent tests, polluters, locations from TestFrames, identity, repro, and the score.
mod support;

use cupel_core::syntax::{Span, TestCase, TestFile, TestFrames};
use cupel_core::{
    ActionKind, CheckStatus, CostClass, FactKind, Facts, Finding, FindingId, ProducerId, RunOrder,
    Scope, Severity, TestOutcome, TestRerun, TestReruns, TestResult,
};
use serde_json::{Value, json};
use support::{Recorded, check, recorded, setting_error};

const ID: &str = "tests.flaky";

const PASS: TestOutcome = TestOutcome::Passed;
const FAIL: TestOutcome = TestOutcome::Failed;

/// A rerun in `order` with `seed` whose tests ran in `executed` order (when given), each test
/// `(id, outcome)` in file `t.py` named after the last `::` part of its ID.
fn rerun(
    order: RunOrder,
    seed: u64,
    executed: Option<&[&str]>,
    tests: &[(&str, TestOutcome)],
) -> TestRerun {
    TestRerun {
        order,
        seed,
        parallelism: Some(1),
        executed: executed.map(|ids| ids.iter().map(|id| (*id).to_owned()).collect()),
        invocations: Vec::new(),
        tests: tests
            .iter()
            .map(|(id, outcome)| TestResult {
                id: (*id).to_owned(),
                file: Some("t.py".to_owned()),
                name: id.rsplit("::").next().unwrap().to_owned(),
                outcome: *outcome,
                duration_ms: None,
                output: outcome.is_failure().then(|| "assert failed".to_owned()),
            })
            .collect(),
    }
}

fn reruns(runs: Vec<TestRerun>) -> TestReruns {
    TestReruns {
        runner: ProducerId::from("tool.pytest"),
        seed: 7,
        runs,
    }
}

/// `t.py` with a case per name, the `n`th starting on line `10 * (n + 1)`.
fn frames(names: &[&str]) -> TestFrames {
    TestFrames {
        files: vec![TestFile {
            file: "t.py".to_owned(),
            framework: "pytest".to_owned(),
            setup: Vec::new(),
            cases: names
                .iter()
                .zip(1u32..)
                .map(|(name, n)| TestCase {
                    name: (*name).to_owned(),
                    span: Span {
                        line: 10 * n,
                        col: 1,
                        end_line: 10 * n + 2,
                        end_col: 5,
                    },
                    assertions: Vec::new(),
                    mocks: Vec::new(),
                    skip: None,
                })
                .collect(),
        }],
    }
}

fn run_over(reruns: TestReruns, frames: TestFrames) -> (cupel_core::CheckResult, Recorded) {
    let check = check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(reruns).unwrap();
    facts.insert(frames).unwrap();
    recorded(check, &facts, Scope::default(), json!({})).unwrap()
}

/// Each finding as (subject test ID from evidence, kind).
fn found(findings: &[Finding]) -> Vec<(String, String)> {
    findings
        .iter()
        .map(|f| {
            (
                f.evidence.0["test"].as_str().unwrap().to_owned(),
                f.kind.clone(),
            )
        })
        .collect()
}

fn pair(test: &str, kind: &str) -> (String, String) {
    (test.to_owned(), kind.to_owned())
}

/// Three reruns of tests `a` and `b`: two in the declared order `[a, b]`, then reversed, with
/// `a`'s outcome in each as given and `b` always passing.
fn three_reruns(a: &str, b: &str, outcomes: [TestOutcome; 3]) -> TestReruns {
    let orders = [
        (RunOrder::Declared, [a, b]),
        (RunOrder::Declared, [a, b]),
        (RunOrder::Reversed, [b, a]),
    ];
    reruns(
        orders
            .iter()
            .zip(outcomes)
            .zip(1..)
            .map(|(((order, executed), outcome), seed)| {
                rerun(*order, seed, Some(executed), &[(a, outcome), (b, PASS)])
            })
            .collect(),
    )
}

/// A victim that passes before the polluter and fails after it.
fn order_dependent() -> TestReruns {
    three_reruns(
        "t.py::test_victim",
        "t.py::test_polluter",
        [PASS, PASS, FAIL],
    )
}

/// A test whose outcome changes between the two declared-order reruns.
fn flaky() -> TestReruns {
    three_reruns("t.py::test_coin", "t.py::test_steady", [FAIL, PASS, FAIL])
}

/// `fact` with no file named for any test, as `go test` reports them.
fn without_files(mut fact: TestReruns) -> TestReruns {
    for test in fact.runs.iter_mut().flat_map(|run| &mut run.tests) {
        test.file = None;
    }
    fact
}

#[test]
fn the_manifest_needs_reruns_and_test_frames_at_standard_cost() {
    let manifest = check(ID).manifest();
    assert_eq!(manifest.needs, [FactKind::TestReruns, FactKind::TestFrames]);
    assert_eq!(manifest.cost, CostClass::Standard);
    assert_eq!(u8::from(manifest.tier), 2);
    let kinds: Vec<(&str, ActionKind)> = manifest
        .findings
        .iter()
        .map(|spec| (spec.kind.as_str(), spec.action))
        .collect();
    assert_eq!(
        kinds,
        [
            ("test.flaky", ActionKind::InvestigateFlake),
            ("test.order-dependent", ActionKind::InvestigateFlake),
        ]
    );
}

#[test]
fn a_test_that_fails_only_after_another_is_order_dependent_and_names_its_polluter() {
    let (result, recorded) = run_over(order_dependent(), frames(&["test_victim", "test_polluter"]));

    assert_eq!(
        found(&recorded.findings),
        [pair("t.py::test_victim", "test.order-dependent")]
    );
    let finding = &recorded.findings[0];
    assert_eq!(
        Value::Object(finding.evidence.0.clone()),
        json!({
            "test": "t.py::test_victim",
            "runner": "tool.pytest",
            "reruns": 3,
            "passed": 2,
            "failed": 1,
            "outcomes": ["passed", "passed", "failed"],
            "orders": ["declared", "declared", "reversed"],
            "preceded_by": ["t.py::test_polluter"],
        })
    );
    let location = finding.location.as_ref().unwrap();
    assert_eq!((location.file.as_str(), location.line), ("t.py", 10));
    assert_eq!(finding.severity, Severity::Warn);
    assert_eq!(finding.action.kind, ActionKind::InvestigateFlake);
    assert_eq!(result.status, CheckStatus::Fail);
}

#[test]
fn a_test_whose_outcome_changes_in_the_same_order_is_flaky_not_order_dependent() {
    let (_, recorded) = run_over(flaky(), frames(&["test_coin", "test_steady"]));

    assert_eq!(
        found(&recorded.findings),
        [pair("t.py::test_coin", "test.flaky")]
    );
    let evidence = &recorded.findings[0].evidence.0;
    assert_eq!(evidence["same_order"], json!([0, 1]));
    assert_eq!(evidence["outcomes"], json!(["failed", "passed", "failed"]));
    assert!(recorded.findings[0].confidence > 0.9);
}

#[test]
fn stable_and_skipped_tests_report_nothing_and_the_score_counts_unstable_tests() {
    let (a, b) = ("t.py::test_a", "t.py::test_b");
    let skipped = TestOutcome::Skipped;
    let steady = reruns(vec![
        rerun(RunOrder::Declared, 1, None, &[(a, PASS), (b, skipped)]),
        rerun(RunOrder::Shuffled, 2, None, &[(a, PASS), (b, PASS)]),
    ]);
    let (result, recorded) = run_over(steady, frames(&["test_a", "test_b"]));
    assert!(recorded.findings.is_empty());
    assert_eq!(result.status, CheckStatus::Pass);
    let score = result.score.unwrap();
    assert_eq!(
        (score.value, score.unit.as_str(), score.n),
        (0.0, "unstable", 2)
    );

    let (result, _) = run_over(order_dependent(), frames(&[]));
    let counted = (
        result.score.unwrap().value,
        result.metrics["order_dependent"],
        result.metrics["flaky"],
    );
    assert_eq!(format!("{counted:?}"), "(1.0, 1.0, 0.0)");
}

#[test]
fn without_a_reported_order_reruns_compare_by_the_order_they_asked_for() {
    let t = "t.py::test_t";
    let by_order = reruns(vec![
        rerun(RunOrder::Declared, 1, None, &[(t, PASS)]),
        rerun(RunOrder::Declared, 2, None, &[(t, PASS)]),
        rerun(RunOrder::Shuffled, 3, None, &[(t, FAIL)]),
    ]);
    let (_, recorded) = run_over(by_order, frames(&["test_t"]));
    assert_eq!(found(&recorded.findings), [pair(t, "test.order-dependent")]);
    let finding = &recorded.findings[0];
    assert_eq!(finding.evidence.0["preceded_by"], json!([]));
    assert!(finding.confidence < 0.9, "no polluter is named");

    let declared_twice = reruns(vec![
        rerun(RunOrder::Declared, 1, None, &[(t, PASS)]),
        rerun(RunOrder::Declared, 2, None, &[(t, FAIL)]),
    ]);
    let (_, recorded) = run_over(declared_twice, frames(&["test_t"]));
    assert_eq!(found(&recorded.findings), [pair(t, "test.flaky")]);
}

#[test]
fn a_test_with_no_file_is_located_by_its_unique_name_in_test_frames() {
    let named = frames(&["test_polluter", "test_victim"]);
    let (_, recorded) = run_over(without_files(order_dependent()), named);
    let location = recorded.findings[0].location.as_ref().unwrap();
    assert_eq!((location.file.as_str(), location.line), ("t.py", 20));

    let (_, recorded) = run_over(without_files(order_dependent()), frames(&[]));
    assert_eq!(recorded.findings[0].location, None);
}

#[test]
fn findings_are_identified_by_test_and_reproduced_with_the_reruns_seed() {
    let (_, recorded) = run_over(order_dependent(), frames(&["test_victim"]));
    let finding = &recorded.findings[0];
    assert_eq!(
        finding.id,
        FindingId::for_subject(
            &check(ID).manifest().id,
            "test.order-dependent",
            "t.py::test_victim"
        )
    );
    assert_eq!(finding.repro, "cupel check --only tests.flaky --seed 7");
    assert!(finding.title.contains("test_victim"), "{}", finding.title);
}

#[test]
fn no_reruns_is_a_pass_over_no_tests() {
    let (result, recorded) = run_over(reruns(Vec::new()), frames(&["test_a"]));
    assert!(recorded.findings.is_empty());
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.score.unwrap().n, 0);
}

#[test]
fn the_check_takes_no_settings() {
    let (key, valid) = setting_error(check(ID), json!({"runs": 5}));
    assert_eq!((key.as_str(), valid), ("runs", Vec::<String>::new()));
}
