// ABOUTME: static.api-surface over synthetic base and head ApiSurface facts: removals, changed and extended
// ABOUTME: signatures, additions, matching by path, identity, location, settings, the score, and the manifest (ADR-101).
mod support;

use cupel_core::syntax::{ApiItem, ApiItemKind, ApiSurface, Span};
use cupel_core::{
    ActionKind, CheckId, CheckResult, CheckStatus, CostClass, FactKind, Facts, Finding, FindingId,
    MetricPer, Produced, Range, Scope, Severity,
};
use serde_json::json;
use support::{Shape, combined, language, score};

const ID: &str = "static.api-surface";

fn item(path: &str, kind: ApiItemKind, signature: &str, file: &str, line: u32) -> ApiItem {
    ApiItem {
        path: path.to_owned(),
        kind,
        signature: signature.to_owned(),
        file: file.to_owned(),
        span: Span {
            line,
            col: 1,
            end_line: line + 2,
            end_col: 2,
        },
    }
}

fn function(path: &str, signature: &str, line: u32) -> ApiItem {
    item(path, ApiItemKind::Function, signature, "lib.x", line)
}

fn surface(items: Vec<ApiItem>) -> ApiSurface {
    ApiSurface { items }
}

/// Runs the check comparing `base` with `head` over a range starting at `c0ffee`.
fn run(base: Vec<ApiItem>, head: Vec<ApiItem>) -> (CheckResult, Vec<Finding>) {
    let check = support::check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(surface(head)).unwrap();
    facts
        .insert_base_shared(&Produced::new(surface(base)).share())
        .unwrap();
    facts.set_range(Range {
        commit: "c0ffee".to_owned(),
        changes: Vec::new(),
    });
    let scope = Scope::of_files(vec!["lib.x".to_owned()]);
    let (result, recorded) = support::recorded(check, &facts, scope, json!({})).unwrap();
    (result, recorded.findings)
}

/// Each finding as (kind, path, change).
fn reported(findings: &[Finding]) -> Vec<(&str, &str, &str)> {
    findings
        .iter()
        .map(|finding| {
            let evidence = &finding.evidence.0;
            (
                finding.kind.as_str(),
                evidence["path"].as_str().unwrap(),
                evidence["change"].as_str().unwrap(),
            )
        })
        .collect()
}

#[test]
fn an_unchanged_surface_reports_nothing_and_scores_zero_over_the_paths_compared() {
    let items = || vec![function("m.a", "(_: int)", 1), function("m.b", "()", 5)];
    let (result, findings) = run(items(), items());
    assert_eq!(findings, []);
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(
        result.metrics,
        [("breaking".to_owned(), 0.0), ("widened".to_owned(), 0.0)].into()
    );
    assert_eq!(result.score, Some(score(0.0, "breaking", 2)));
}

#[test]
fn a_removed_item_is_breaking_and_located_where_it_was_at_the_base() {
    let base = vec![function("m.a", "(_: int)", 1), function("m.gone", "()", 9)];
    let head = vec![function("m.a", "(_: int)", 1)];
    let (result, findings) = run(base, head);
    assert_eq!(reported(&findings), [("api.breaking", "m.gone", "removed")]);
    let finding = &findings[0];
    assert_eq!(finding.severity, Severity::Warn);
    assert_eq!(finding.action.kind, ActionKind::Review);
    assert_eq!(finding.location.as_ref().unwrap().line, 9);
    assert_eq!(
        finding.evidence.0,
        *json!({
            "path": "m.gone",
            "change": "removed",
            "base": [{"kind": "function", "signature": "()"}],
            "head": [],
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(
        finding.id,
        FindingId::for_subject(&CheckId::from(ID), "api.breaking", "m.gone")
    );
    assert_eq!(
        finding.repro,
        "cupel check --only static.api-surface --base c0ffee"
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.metrics.get("breaking"), Some(&1.0));
    assert_eq!(result.score, Some(score(1.0, "breaking", 2)));
}

#[test]
fn a_changed_signature_or_kind_is_breaking_and_located_at_head() {
    let base = vec![
        function("m.sig", "(_: int) -> int", 1),
        function("m.kind", "", 5),
    ];
    let head = vec![
        function("m.sig", "(_: int, _: int) -> int", 3),
        item("m.kind", ApiItemKind::Constant, "", "lib.x", 7),
    ];
    let (_, findings) = run(base, head);
    assert_eq!(
        reported(&findings),
        [
            ("api.breaking", "m.sig", "changed"),
            ("api.breaking", "m.kind", "changed"),
        ]
    );
    assert_eq!(findings[0].location.as_ref().unwrap().line, 3);
    assert_eq!(
        findings[0].evidence.0["head"],
        json!([{"kind": "function", "signature": "(_: int, _: int) -> int"}])
    );
    assert_eq!(findings[1].location.as_ref().unwrap().line, 7);
}

#[test]
fn an_added_item_and_an_added_overload_widen_the_surface_without_failing() {
    let base = vec![function("m.f", "(_: string)", 1)];
    let head = vec![
        function("m.f", "(_: string)", 1),
        function("m.f", "(_: number)", 2),
        item("m.T", ApiItemKind::Type, "", "other.x", 4),
    ];
    let (result, findings) = run(base, head);
    assert_eq!(
        reported(&findings),
        [
            ("api.widened", "m.f", "extended"),
            ("api.widened", "m.T", "added"),
        ]
    );
    assert!(findings.iter().all(|f| f.severity == Severity::Info));
    assert_eq!(findings[1].location.as_ref().unwrap().file, "other.x");
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(
        result.metrics,
        [("breaking".to_owned(), 0.0), ("widened".to_owned(), 2.0)].into()
    );
}

#[test]
fn an_item_moved_to_another_file_under_its_path_is_unchanged() {
    let base = vec![item("m.f", ApiItemKind::Function, "()", "a.x", 1)];
    let head = vec![item("m.f", ApiItemKind::Function, "()", "a/mod.x", 30)];
    let (_, findings) = run(base, head);
    assert_eq!(findings, []);
}

#[test]
fn a_lost_overload_is_breaking_even_when_another_is_added() {
    let base = vec![function("m.f", "(_: string)", 1)];
    let head = vec![function("m.f", "(_: number)", 1)];
    let (_, findings) = run(base, head);
    assert_eq!(reported(&findings), [("api.breaking", "m.f", "changed")]);
}

#[test]
fn findings_follow_base_order_then_head_order_for_additions() {
    let base = vec![function("m.z", "()", 1), function("m.a", "()", 2)];
    let head = vec![function("m.new2", "()", 1), function("m.new1", "()", 2)];
    let (_, findings) = run(base, head);
    assert_eq!(
        reported(&findings),
        [
            ("api.breaking", "m.z", "removed"),
            ("api.breaking", "m.a", "removed"),
            ("api.widened", "m.new2", "added"),
            ("api.widened", "m.new1", "added"),
        ]
    );
}

#[test]
fn settings_are_refused() {
    let (key, valid) = support::setting_error(support::check(ID), json!({"strict": 1}));
    assert_eq!(key, "strict");
    assert!(valid.is_empty());
}

#[test]
fn breaking_and_widened_add_up_across_languages() {
    let check = support::check(ID);
    let results = [
        language(check, &[("breaking", 2.0), ("widened", 1.0)], 4),
        language(check, &[("breaking", 1.0), ("widened", 0.0)], 3),
    ];
    assert_eq!(combined(check, "breaking", &results), Some(3.0));
    assert_eq!(combined(check, "widened", &results), Some(1.0));
    assert_eq!(
        check.combine(&[score(2.0, "breaking", 4), score(1.0, "breaking", 3)]),
        Some(score(3.0, "breaking", 7))
    );
}

#[test]
fn the_manifest_needs_api_surface_on_both_sides() {
    let check = support::check(ID);
    assert_eq!(check.base_needs(), [FactKind::ApiSurface]);
    assert_eq!(
        Shape::of(check.manifest()),
        Shape {
            needs: vec![FactKind::ApiSurface],
            wants: vec![],
            tier: 0,
            cost: CostClass::Quick,
            kinds: vec![
                ("api.breaking", ActionKind::Review),
                ("api.widened", ActionKind::Review),
            ],
            metrics: vec![
                ("breaking", MetricPer::Scope),
                ("widened", MetricPer::Scope)
            ],
            evidence: vec!["object", "object"],
        }
    );
}
