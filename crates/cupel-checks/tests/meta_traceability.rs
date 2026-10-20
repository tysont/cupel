// ABOUTME: meta.traceability over synthetic base and head TestFrames: requirement IDs matched in test names, a
// ABOUTME: requirement traced only by a test the range added, the finding and its evidence, settings, and the score (ADR-114).
mod support;

use cupel_core::syntax::{Span, TestCase, TestFile, TestFrames};
use cupel_core::{
    ActionKind, CheckId, CheckResult, CheckStatus, CostClass, FactKind, Facts, Finding, FindingId,
    MetricPer, Produced, Range, Scope, Severity,
};
use serde_json::{Value, json};
use support::{Shape, score};

const ID: &str = "meta.traceability";

fn case(name: &str, line: u32) -> TestCase {
    TestCase {
        name: name.to_owned(),
        span: Span {
            line,
            col: 1,
            end_line: line + 2,
            end_col: 2,
        },
        assertions: Vec::new(),
        mocks: Vec::new(),
        skip: None,
        property: None,
        doc: None,
    }
}

fn frames(file: &str, names: &[&str]) -> TestFrames {
    TestFrames {
        files: vec![TestFile {
            file: file.to_owned(),
            framework: "pytest".to_owned(),
            setup: Vec::new(),
            cases: names
                .iter()
                .enumerate()
                .map(|(index, name)| case(name, 1 + 4 * u32::try_from(index).unwrap()))
                .collect(),
        }],
    }
}

/// The check over `head` and `base` with `config`.
fn traced(head: TestFrames, base: TestFrames, config: Value) -> (CheckResult, Vec<Finding>) {
    let check = support::check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(head).unwrap();
    facts
        .insert_base_shared(&Produced::new(base).share())
        .unwrap();
    facts.set_range(Range {
        commit: "0a1b2c3".to_owned(),
        changes: Vec::new(),
    });
    let scope = Scope::of_files(vec!["tests/test_dates.py".to_owned()]);
    let (result, recorded) = support::recorded(check, &facts, scope, config).unwrap();
    (result, recorded.findings)
}

fn requirements(ids: &[&str]) -> Value {
    json!({ "requirements": ids })
}

#[test]
fn a_requirement_is_traced_by_a_test_the_range_added_whose_name_carries_its_id() {
    let base = frames("tests/test_dates.py", &["test_parses_iso"]);
    let head = frames(
        "tests/test_dates.py",
        &[
            "test_parses_iso",
            "test_req_12_parses_week_dates",
            "parses leap days #REQ-13",
        ],
    );
    let (result, findings) = traced(head, base, requirements(&["REQ-12", "REQ-13"]));
    assert_eq!(findings, []);
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.metrics, [("untested".to_owned(), 0.0)].into());
    assert_eq!(result.score, Some(score(0.0, "untested requirements", 2)));
}

#[test]
fn a_requirement_only_a_pre_existing_test_names_is_untested_and_its_evidence_says_so() {
    let base = frames("tests/test_dates.py", &["test_req_12_parses"]);
    let head = frames(
        "tests/test_dates.py",
        &["test_req_12_parses", "test_parses_iso"],
    );
    let (result, findings) = traced(head, base, requirements(&["REQ-12", "REQ-14"]));
    assert_eq!(findings.len(), 2);
    let pre_existing = &findings[0];
    assert_eq!(
        (
            pre_existing.kind.as_str(),
            pre_existing.severity,
            pre_existing.action.kind
        ),
        (
            "spec.untested-requirement",
            Severity::Warn,
            ActionKind::WriteTest
        )
    );
    assert_eq!(
        pre_existing.title,
        "requirement REQ-12 has no test the change added; 1 test that existed before names it"
    );
    assert_eq!(
        serde_json::to_value(&pre_existing.evidence).unwrap(),
        json!({
            "requirement": "REQ-12",
            "base": "0a1b2c3",
            "tests": [{"file": "tests/test_dates.py", "name": "test_req_12_parses", "added": false, "line": 1}],
        })
    );
    assert_eq!(
        pre_existing.id,
        FindingId::for_subject(&CheckId::from(ID), "spec.untested-requirement", "REQ-12")
    );
    assert_eq!(pre_existing.location, None);
    assert_eq!(
        pre_existing.repro,
        "cupel check --changed --only meta.traceability"
    );
    assert_eq!(
        findings[1].title,
        "requirement REQ-14 has no test that names it"
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.metrics, [("untested".to_owned(), 2.0)].into());
    assert_eq!(result.score, Some(score(2.0, "untested requirements", 2)));
}

#[test]
fn an_id_matches_whole_tokens_in_any_case_and_separator() {
    let head = frames(
        "tests/test_dates.py",
        &[
            "test_req_120_other",
            "TestREQ1Camel",
            "it(\"req 1: parses\")",
        ],
    );
    let (_, findings) = traced(
        head,
        frames("x.py", &[]),
        requirements(&["REQ-1", "REQ-120"]),
    );
    assert_eq!(findings, []);
    let head = frames(
        "tests/test_dates.py",
        &["test_req_120_other", "TestREQ1Camel"],
    );
    let (_, findings) = traced(head, frames("x.py", &[]), requirements(&["REQ-1"]));
    assert_eq!(
        findings.len(),
        1,
        "REQ-1 is neither REQ-120 nor inside a camel-case word"
    );
}

#[test]
fn without_requirements_nothing_is_traced_and_a_note_says_how_to_list_them() {
    let (result, findings) = traced(
        frames("tests/test_dates.py", &["test_a"]),
        frames("tests/test_dates.py", &[]),
        json!({}),
    );
    assert_eq!(findings, []);
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.score, None);
    assert!(result.metrics.is_empty());
    assert_eq!(
        result.notes,
        [
            "no requirements are configured: list their IDs as `requirements` under \
          [checks.\"meta.traceability\"]"
        ]
    );
}

#[test]
fn requirements_must_be_a_list_of_ids() {
    let check = support::check(ID);
    for config in [
        json!({"requirements": "REQ-1"}),
        json!({"requirements": [1]}),
        json!({"requirements": [""]}),
    ] {
        let (key, valid) = support::setting_error(check, config);
        assert_eq!(
            (key.as_str(), valid),
            ("requirements", vec!["requirements".to_owned()])
        );
    }
    let (key, _) = support::setting_error(check, json!({"other": 1}));
    assert_eq!(key, "other");
    assert_eq!(
        support::declared_settings(ID),
        [("requirements", json!([]))]
    );
}

#[test]
fn untested_requirements_add_up_across_languages() {
    support::assert_counts_add_up(ID, &["untested"], "untested requirements");
}

#[test]
fn the_manifest_reads_test_frames_on_both_sides_of_the_range() {
    let check = support::check(ID);
    assert_eq!(check.base_needs(), [FactKind::TestFrames]);
    assert_eq!(
        Shape::of(check.manifest()),
        Shape {
            needs: vec![FactKind::TestFrames],
            wants: vec![],
            tier: 2,
            cost: CostClass::Standard,
            kinds: vec![("spec.untested-requirement", ActionKind::WriteTest)],
            metrics: vec![("untested", MetricPer::Scope)],
            evidence: vec!["object"],
        }
    );
}
