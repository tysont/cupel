// ABOUTME: tests.weakening over synthetic base and head TestFrames: matching (repeats, moves), the five
// ABOUTME: kinds and their combinations, snapshot attribution, identity, settings, and the score (ADR-057).
mod support;

use cupel_core::git::{Change, ChangeKind};
use cupel_core::syntax::{
    Assertion, AssertionKind, AssertionStrength, SkipMarker, Span, TestCase, TestFile, TestFrames,
};
use cupel_core::{
    Check, CheckId, CheckResult, CheckStatus, Error, Facts, Finding, FindingId, Produced, Range,
    Scope, Score,
};
use serde_json::{Value, json};
use support::{combined, language};

const ID: &str = "tests.weakening";

fn span(line: u32) -> Span {
    Span {
        line,
        col: 1,
        end_line: line + 2,
        end_col: 2,
    }
}

fn assertion(kind: AssertionKind, strength: AssertionStrength) -> Assertion {
    Assertion {
        kind,
        strength,
        span: span(1),
    }
}

fn strong() -> Assertion {
    assertion(AssertionKind::Equality, AssertionStrength::Strong)
}

fn weak() -> Assertion {
    assertion(AssertionKind::Truthiness, AssertionStrength::Weak)
}

fn snapshot() -> Assertion {
    assertion(AssertionKind::Snapshot, AssertionStrength::Strong)
}

fn case(name: &str, line: u32, assertions: Vec<Assertion>) -> TestCase {
    TestCase {
        name: name.to_owned(),
        span: span(line),
        assertions,
        mocks: Vec::new(),
        skip: None,
        property: None,
    }
}

fn skipped(mut case: TestCase, reason: &str) -> TestCase {
    case.skip = Some(SkipMarker {
        reason: Some(reason.to_owned()),
        span: span(case.span.line - 1),
    });
    case
}

fn frames(files: Vec<(&str, Vec<TestCase>)>) -> TestFrames {
    TestFrames {
        files: files
            .into_iter()
            .map(|(file, cases)| TestFile {
                file: file.to_owned(),
                framework: "x".to_owned(),
                setup: Vec::new(),
                cases,
            })
            .collect(),
    }
}

fn modified(path: &str) -> Change {
    Change {
        path: path.to_owned(),
        kind: ChangeKind::Modified,
    }
}

/// Runs the check comparing `base` with `head` over a range with `changes`.
fn run(base: TestFrames, head: TestFrames, changes: Vec<Change>) -> (CheckResult, Vec<Finding>) {
    let check = support::check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(head).unwrap();
    facts
        .insert_base_shared(&Produced::new(base).share())
        .unwrap();
    facts.set_range(Range {
        commit: "c0ffee".to_owned(),
        changes,
    });
    run_with(check, &facts).unwrap()
}

fn run_with(check: &dyn Check, facts: &Facts) -> cupel_core::Result<(CheckResult, Vec<Finding>)> {
    let scope = Scope::of_files(vec!["t.x".to_owned()]);
    let (result, recorded) = support::recorded(check, facts, scope, json!({}))?;
    Ok((result, recorded.findings))
}

fn kinds(findings: &[Finding]) -> Vec<(&str, &str)> {
    findings
        .iter()
        .map(|finding| {
            let evidence = &finding.evidence.0;
            let subject = evidence
                .get("test")
                .or_else(|| evidence.get("snapshot"))
                .and_then(Value::as_str)
                .unwrap();
            (finding.kind.as_str(), subject)
        })
        .collect()
}

// ---------------------------------------------------------------- matching

#[test]
fn an_unchanged_suite_reports_nothing_and_scores_zero_over_the_tests_compared() {
    let suite = || {
        frames(vec![(
            "t.x",
            vec![case("a", 1, vec![strong()]), case("b", 5, vec![weak()])],
        )])
    };
    let (result, findings) = run(suite(), suite(), vec![]);
    assert_eq!(findings, []);
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.metrics, [("weakened".to_owned(), 0.0)].into());
    assert_eq!(
        result.score,
        Some(Score {
            value: 0.0,
            unit: "weakened".to_owned(),
            n: 2
        })
    );
}

#[test]
fn repeated_names_pair_in_order_so_only_the_weakened_repeat_is_reported() {
    let base = frames(vec![(
        "t.x",
        vec![
            case("same", 1, vec![strong()]),
            case("same", 5, vec![strong(), strong()]),
        ],
    )]);
    let head = frames(vec![(
        "t.x",
        vec![
            case("same", 1, vec![strong()]),
            case("same", 5, vec![strong()]),
        ],
    )]);
    let (_, findings) = run(base, head, vec![]);
    assert_eq!(kinds(&findings), [("assertion.removed", "same")]);
    let check = CheckId::from(ID);
    assert_eq!(
        findings[0].id,
        FindingId::for_subject(&check, "assertion.removed", "t.x::same#1")
    );
    assert_eq!(findings[0].location.as_ref().unwrap().line, 5);
}

#[test]
fn a_test_moved_to_another_file_is_compared_there_and_not_deleted() {
    let base = frames(vec![(
        "old.x",
        vec![case("moves", 1, vec![strong(), strong()])],
    )]);
    let head = frames(vec![("new.x", vec![case("moves", 9, vec![strong()])])]);
    let (_, findings) = run(base, head, vec![]);
    assert_eq!(kinds(&findings), [("assertion.removed", "moves")]);
    assert_eq!(findings[0].location.as_ref().unwrap().file, "new.x");
}

#[test]
fn a_deleted_test_is_located_where_it_was_at_the_base() {
    let base = frames(vec![(
        "t.x",
        vec![
            case("kept", 1, vec![strong()]),
            case("gone", 7, vec![strong()]),
        ],
    )]);
    let head = frames(vec![("t.x", vec![case("kept", 1, vec![strong()])])]);
    let (result, findings) = run(base, head, vec![]);
    assert_eq!(kinds(&findings), [("test.deleted", "gone")]);
    assert_eq!(
        findings[0].evidence.0,
        json!({"test": "gone", "file": "t.x", "line": 7})
            .as_object()
            .unwrap()
            .clone()
    );
    assert_eq!(
        findings[0].repro,
        "cupel check --only tests.weakening --base c0ffee"
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.findings, 1);
}

// ---------------------------------------------------------------- kinds

#[test]
fn trading_exact_assertions_for_fewer_weak_ones_is_both_removed_and_loosened() {
    let base = frames(vec![(
        "t.x",
        vec![case("t", 1, vec![strong(), strong(), strong()])],
    )]);
    let head = frames(vec![("t.x", vec![case("t", 1, vec![weak(), strong()])])]);
    let (result, findings) = run(base, head, vec![]);
    assert_eq!(
        kinds(&findings),
        [("assertion.removed", "t"), ("matcher.loosened", "t")]
    );
    assert_eq!(
        result.metrics,
        [("weakened".to_owned(), 2.0)].into(),
        "the gate key counts every weakening"
    );
    assert_eq!(
        Value::Object(findings[1].evidence.0.clone()),
        json!({"test": "t", "strong_base": 3, "strong_head": 1, "weak_base": 0, "weak_head": 1})
    );
}

#[test]
fn strengthening_or_reordering_assertions_is_not_a_weakening() {
    let base = frames(vec![("t.x", vec![case("t", 1, vec![weak(), strong()])])]);
    let head = frames(vec![(
        "t.x",
        vec![case("t", 1, vec![strong(), strong(), weak()])],
    )]);
    let (_, findings) = run(base, head, vec![]);
    assert_eq!(findings, []);
}

#[test]
fn only_a_newly_added_skip_is_reported() {
    let base = frames(vec![(
        "t.x",
        vec![
            case("fresh", 2, vec![strong()]),
            skipped(case("old", 6, vec![strong()]), "known"),
        ],
    )]);
    let head = frames(vec![(
        "t.x",
        vec![
            skipped(case("fresh", 2, vec![strong()]), "flaky"),
            skipped(case("old", 6, vec![strong()]), "known"),
        ],
    )]);
    let (_, findings) = run(base, head, vec![]);
    assert_eq!(kinds(&findings), [("test.skipped", "fresh")]);
    assert_eq!(findings[0].evidence.0["reason"], json!("flaky"));
}

// ---------------------------------------------------------------- snapshots

#[test]
fn a_modified_snapshot_named_after_a_test_is_attributed_to_that_test_only() {
    let suite = || {
        frames(vec![(
            "tests/view.x",
            vec![
                case("renders", 1, vec![snapshot()]),
                case("prints", 5, vec![snapshot()]),
            ],
        )])
    };
    let (_, findings) = run(
        suite(),
        suite(),
        vec![
            modified("tests/snapshots/view__renders.snap"),
            modified("README.md"),
        ],
    );
    assert_eq!(
        kinds(&findings),
        [("snapshot.rewritten", "tests/snapshots/view__renders.snap")]
    );
    assert_eq!(findings[0].evidence.0["tests"], json!(["renders"]));
    assert_eq!(findings[0].location.as_ref().unwrap().file, "tests/view.x");
}

#[test]
fn a_snapshot_no_changed_test_claims_is_located_at_the_snapshot() {
    let (_, findings) = run(
        frames(vec![]),
        frames(vec![]),
        vec![
            modified("web/__snapshots__/page.test.js.snap"),
            Change {
                path: "web/__snapshots__/new.test.js.snap".to_owned(),
                kind: ChangeKind::Added,
            },
            modified("data/fixture.json"),
        ],
    );
    assert_eq!(
        kinds(&findings),
        [("snapshot.rewritten", "web/__snapshots__/page.test.js.snap")]
    );
    let location = findings[0].location.as_ref().unwrap();
    assert_eq!(
        (location.file.as_str(), location.line),
        ("web/__snapshots__/page.test.js.snap", 1)
    );
    assert_eq!(findings[0].evidence.0["tests"], json!([]));
}

#[test]
fn a_snapshot_file_shared_with_a_new_test_only_grew() {
    let base = frames(vec![(
        "web/page.test.js",
        vec![case("old", 1, vec![snapshot()])],
    )]);
    let head = frames(vec![(
        "web/page.test.js",
        vec![
            case("old", 1, vec![snapshot()]),
            case("new", 5, vec![snapshot()]),
        ],
    )]);
    let (_, findings) = run(
        base,
        head,
        vec![modified("web/__snapshots__/page.test.js.snap")],
    );
    assert_eq!(findings, []);
}

// ---------------------------------------------------------------- contract

#[test]
fn the_check_reads_the_base_side_of_test_frames_and_has_no_settings() {
    let check = support::check(ID);
    assert_eq!(check.base_needs(), [cupel_core::FactKind::TestFrames]);
    assert_eq!(check.manifest().needs, [cupel_core::FactKind::TestFrames]);
    let (ctx, _dir) = support::ctx(
        Scope::of_files(vec!["t.x".to_owned()]),
        json!({"threshold": 1}),
    );
    let error = check.plan(&ctx, &ctx.scope).unwrap_err();
    assert!(matches!(error, Error::Config(_)), "{error:?}");
}

#[test]
fn without_a_base_side_the_check_reports_the_absent_fact() {
    let check = support::check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(frames(vec![])).unwrap();
    assert_eq!(
        run_with(check, &facts).unwrap_err(),
        Error::AbsentFact {
            kind: cupel_core::FactKind::TestFrames
        }
    );
}

#[test]
fn languages_combine_by_summing_weakenings() {
    let check = support::check(ID);
    let score = |value, n| Score {
        value,
        unit: "weakened".to_owned(),
        n,
    };
    assert_eq!(
        check.combine(&[score(2.0, 10), score(1.0, 1)]),
        Some(score(3.0, 11))
    );
    let results = [
        language(check, &[("weakened", 2.0)], 10),
        language(check, &[("weakened", 1.0)], 1),
    ];
    assert_eq!(combined(check, "weakened", &results), Some(3.0));
}
