// ABOUTME: tests.weakening over synthetic base and head TestFrames: matching (repeats, moves, renames, splits),
// ABOUTME: the five kinds and their combinations, snapshots, identity, settings, and the score (ADR-057, ADR-131).
mod support;

use std::hash::{DefaultHasher, Hash, Hasher};

use cupel_core::git::{Change, ChangeKind};
use cupel_core::syntax::{
    Assertion, AssertionKind, AssertionStrength, NormNode, NormTree, NormalizedSymbol, SkipMarker,
    Span, SymbolDef, SymbolKind, Symbols, TestCase, TestFile, TestFrames, Visibility,
};
use cupel_core::{
    Check, CheckId, CheckResult, CheckStatus, Error, FactKind, Facts, Finding, FindingId, LangId,
    Produced, Range, Scope, Score,
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
        doc: None,
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

/// Runs the check comparing `base` with `head` over a range with `changes`, no test having a
/// normalized tree to match by.
fn run(base: TestFrames, head: TestFrames, changes: Vec<Change>) -> (CheckResult, Vec<Finding>) {
    run_matched((base, &[]), (head, &[]), changes)
}

/// The tree of a test body of `count` assertions from `first` on, each comparing a call with
/// a literal: twenty assertions make a tree of about a hundred nodes.
fn body(first: usize, count: usize) -> NormNode {
    let assertion = |n: usize| {
        let call = NormNode::new("call", vec![NormNode::leaf("$id"); n % 4 + 1]);
        let expected = NormNode::leaf(if n.is_multiple_of(3) { "$lit" } else { "$id" });
        NormNode::new(
            "macro",
            vec![NormNode::new("arguments", vec![call, expected])],
        )
    };
    NormNode::new("block", (first..first + count).map(assertion).collect())
}

/// A tree of `count` loops over conditions, sharing no shape with `body`.
fn loops(count: usize) -> NormNode {
    let statement = |n: usize| {
        NormNode::new(
            "loop",
            vec![NormNode::new("if", vec![NormNode::leaf("$lit"); n % 4 + 1])],
        )
    };
    NormNode::new("block", (0..count).map(statement).collect())
}

/// The `Symbols` and `NormTree` of `frames`' tests that `trees` gives a tree, by test name.
fn symbols(frames: &TestFrames, trees: &[(&str, NormNode)]) -> (Symbols, NormTree) {
    let mut symbols = Symbols::default();
    let mut normalized = NormTree::default();
    for file in &frames.files {
        for case in &file.cases {
            let Some((_, root)) = trees.iter().find(|(name, _)| *name == case.name) else {
                continue;
            };
            let def = SymbolDef {
                name: case.name.clone(),
                namespace: String::new(),
                display: case.name.clone(),
                kind: SymbolKind::Function,
                visibility: Visibility::Private,
                lang: LangId::from("rust"),
                file: file.file.clone(),
                span: case.span,
                body: None,
                parameters: 0,
            };
            let mut hasher = DefaultHasher::new();
            serde_json::to_string(root).unwrap().hash(&mut hasher);
            normalized.symbols.push(NormalizedSymbol {
                symbol: def.key(),
                hash: format!("sh_{:016x}", hasher.finish()).parse().unwrap(),
                root: root.clone(),
            });
            symbols.symbols.push(def);
        }
    }
    (symbols, normalized)
}

/// Runs the check comparing `base` with `head` over a range with `changes`; each side's tests
/// named in its trees have those normalized trees.
fn run_matched(
    (base, base_trees): (TestFrames, &[(&str, NormNode)]),
    (head, head_trees): (TestFrames, &[(&str, NormNode)]),
    changes: Vec<Change>,
) -> (CheckResult, Vec<Finding>) {
    let check = support::check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    let (head_symbols, head_normalized) = symbols(&head, head_trees);
    let (base_symbols, base_normalized) = symbols(&base, base_trees);
    facts.insert(head).unwrap();
    facts.insert(head_symbols).unwrap();
    facts.insert(head_normalized).unwrap();
    for fact in [
        Produced::new(base).share(),
        Produced::new(base_symbols).share(),
        Produced::new(base_normalized).share(),
    ] {
        facts.insert_base_shared(&fact).unwrap();
    }
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

#[test]
fn a_renamed_test_is_compared_with_its_predecessor_and_not_deleted() {
    let base = frames(vec![(
        "t.x",
        vec![case("checks_totals", 1, vec![strong(), strong()])],
    )]);
    let head = frames(vec![(
        "t.x",
        vec![case("totals_add_up", 1, vec![strong()])],
    )]);
    let (result, findings) = run_matched(
        (base, &[("checks_totals", body(0, 20))]),
        (head, &[("totals_add_up", body(0, 19))]),
        vec![],
    );
    assert_eq!(kinds(&findings), [("assertion.removed", "totals_add_up")]);
    assert_eq!(result.score.map(|score| score.n), Some(1));
}

#[test]
fn a_renamed_test_with_an_unchanged_body_is_not_a_weakening() {
    let base = frames(vec![("t.x", vec![case("old_name", 1, vec![strong()])])]);
    let head = frames(vec![("t.x", vec![case("new_name", 1, vec![strong()])])]);
    let (_, findings) = run_matched(
        (base, &[("old_name", body(0, 20))]),
        (head, &[("new_name", body(0, 20))]),
        vec![],
    );
    assert_eq!(findings, []);
}

#[test]
fn a_test_split_in_two_is_compared_with_the_sum_of_its_parts() {
    let whole = || {
        frames(vec![(
            "t.x",
            vec![case("covers_everything", 1, vec![strong(); 4])],
        )])
    };
    let parts = |first: usize, second: usize| {
        frames(vec![(
            "t.x",
            vec![
                case("covers_loading", 1, vec![strong(); first]),
                case("covers_saving", 9, vec![strong(); second]),
            ],
        )])
    };
    let trees = [
        ("covers_loading", body(0, 12)),
        ("covers_saving", body(12, 12)),
    ];
    let (kept, unchanged) = run_matched(
        (whole(), &[("covers_everything", body(0, 24))]),
        (parts(2, 2), &trees),
        vec![],
    );
    assert_eq!(unchanged, []);
    assert_eq!(kept.score.map(|score| score.n), Some(2));
    let (_, fewer) = run_matched(
        (whole(), &[("covers_everything", body(0, 24))]),
        (parts(2, 1), &trees),
        vec![],
    );
    assert_eq!(kinds(&fewer), [("assertion.removed", "covers_loading")]);
    assert_eq!(
        Value::Object(fewer[0].evidence.0.clone()),
        json!({"test": "covers_loading", "base": 4, "head": 3})
    );
}

#[test]
fn a_deleted_test_unlike_any_added_one_is_still_deleted() {
    let base = frames(vec![("t.x", vec![case("gone", 1, vec![strong()])])]);
    let head = frames(vec![("t.x", vec![case("unrelated", 1, vec![strong()])])]);
    let (_, findings) = run_matched(
        (base, &[("gone", body(0, 20))]),
        (head, &[("unrelated", loops(20))]),
        vec![],
    );
    assert_eq!(kinds(&findings), [("test.deleted", "gone")]);
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
    let sides = [FactKind::TestFrames, FactKind::Symbols, FactKind::NormTree];
    assert_eq!(check.base_needs(), sides);
    assert_eq!(check.manifest().needs, sides);
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
