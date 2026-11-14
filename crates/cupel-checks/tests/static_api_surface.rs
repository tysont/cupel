// ABOUTME: static.api-surface over synthetic base and head ApiSurface facts: removals, changed and extended
// ABOUTME: signatures, additions, matching by path, identity, location, settings, the score, and the manifest (ADR-101).
mod support;

use cupel_core::syntax::{
    ApiItem, ApiItemKind, ApiSurface, EdgeKind, ImportAlias, ImportEdge, ImportTarget, ModuleGraph,
    RootImports, Span, TestConventions,
};
use cupel_core::{
    ActionKind, CheckId, CheckResult, CheckStatus, CostClass, FactKind, Facts, Finding, FindingId,
    MetricPer, Produced, Range, Scope, Severity,
};
use serde_json::json;
use support::{Shape, score};

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
        internal: false,
        is_abstract: false,
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
    run_with(base, head, Vec::new())
}

/// [`run`] with a head module graph of `edges`; files below `test/` are tests.
fn run_with(
    base: Vec<ApiItem>,
    head: Vec<ApiItem>,
    edges: Vec<ImportEdge>,
) -> (CheckResult, Vec<Finding>) {
    let check = support::check(ID);
    let mut facts = Facts::declaring(
        check
            .manifest()
            .needs
            .iter()
            .chain(&check.manifest().wants)
            .copied()
            .chain([FactKind::TestConventions]),
    );
    facts.insert(surface(head)).unwrap();
    facts
        .insert(TestConventions {
            test_globs: vec!["test/**".to_owned()],
            pairs: Vec::new(),
        })
        .unwrap();
    facts
        .insert(ModuleGraph {
            roots: vec![RootImports {
                root: ".".to_owned(),
                edges,
            }],
        })
        .unwrap();
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
        finding.action.hints.get("location_commit"),
        Some(&json!("c0ffee"))
    );
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
    assert_eq!(findings[0].action.hints.get("location_commit"), None);
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

/// `export { names } from "./models"` in `constants.x` at head, `names` each `(name, local)`;
/// none is `export * from`.
fn reexport(names: &[(&str, &str)]) -> ImportEdge {
    ImportEdge {
        from: "constants.x".to_owned(),
        specifier: "./models".to_owned(),
        target: ImportTarget::File {
            path: "models.x".to_owned(),
        },
        relative: true,
        line: 1,
        edge_kind: EdgeKind::ReExport,
        names: names.iter().map(|(name, _)| (*name).to_owned()).collect(),
        aliases: names
            .iter()
            .filter(|(name, local)| name != local)
            .map(|(name, local)| ImportAlias {
                name: (*name).to_owned(),
                local: (*local).to_owned(),
            })
            .collect(),
    }
}

fn constant(path: &str, signature: &str, file: &str) -> ApiItem {
    item(path, ApiItemKind::Constant, signature, file, 1)
}

/// An item moved to another module that its old module re-exports, by name, under an alias,
/// or with `export *`, is still public at its old path: only its new path is added (ADR-174).
#[test]
fn an_item_its_module_re_exports_from_another_is_not_removed() {
    let base = || vec![constant("m.constants.C", "Record", "constants.x")];
    let moved = |name: &str| vec![constant(&format!("m.models.{name}"), "Record", "models.x")];
    for (head, edge) in [
        (moved("C"), reexport(&[("C", "C")])),
        (moved("D"), reexport(&[("D", "C")])),
        (moved("C"), reexport(&[])),
    ] {
        let path = head[0].path.clone();
        let (_, findings) = run_with(base(), head, vec![edge]);
        assert_eq!(
            reported(&findings),
            [("api.widened", path.as_str(), "added")]
        );
    }
}

/// A re-exported item is compared with what it re-exports: another signature is breaking, and
/// a target the range did not change, so not in the head surface, is taken as unchanged.
#[test]
fn a_re_exported_item_is_compared_with_its_target() {
    let base = vec![constant("m.constants.C", "Record", "constants.x")];
    let head = vec![constant("m.models.C", "Map", "models.x")];
    let (_, findings) = run_with(base.clone(), head, vec![reexport(&[("C", "C")])]);
    assert_eq!(
        reported(&findings),
        [
            ("api.breaking", "m.constants.C", "changed"),
            ("api.widened", "m.models.C", "added")
        ]
    );
    let (_, findings) = run_with(base.clone(), Vec::new(), vec![reexport(&[("C", "C")])]);
    assert_eq!(reported(&findings), []);
    let (_, findings) = run_with(base, Vec::new(), vec![reexport(&[("X", "X")])]);
    assert_eq!(
        reported(&findings),
        [("api.breaking", "m.constants.C", "removed")]
    );
}

/// Trailing parameters a caller may leave out (optional, defaulted, or variadic) widen a
/// signature; a required one, a changed one, or another return type is breaking (ADR-174).
#[test]
fn trailing_parameters_a_caller_may_leave_out_widen_a_signature() {
    let changes = |base: &str, head: &str| {
        let (_, findings) = run(
            vec![function("m.f", base, 1)],
            vec![function("m.f", head, 1)],
        );
        reported(&findings)
            .into_iter()
            .map(|(_, _, change)| change.to_owned())
            .collect::<Vec<_>>()
    };
    let base = "(_: string[], _: (x) => void): string | null";
    for head in [
        "(_: string[], _: (x) => void, _: Record<string, [number, number]> = []): string | null",
        "(_: string[], _: (x) => void, _?: \"5m\" | \"1h\", _: Billing = { a: 1, b: 2 }): string | null",
        "(_: string[], _: (x) => void, ..._: number[]): string | null",
    ] {
        assert_eq!(changes(base, head), ["extended"], "{head}");
    }
    assert_eq!(changes("()", "(_=None, *_, **_)"), ["extended"]);
    for head in [
        "(_: string[], _: (x) => void, _: number): string | null",
        "(_: string[], _?: (x) => void): string | null",
        "(_: string[], _: (x) => void, _?: number): string",
    ] {
        assert_eq!(changes(base, head), ["changed"], "{head}");
    }
    assert_eq!(
        changes(
            "(_ *Account) Deposit(_ int64)",
            "(_ *Account) Deposit(_ int64, _ bool)"
        ),
        ["changed"]
    );
}

/// An item no entry of its package exposes is internal (ADR-176): moving, removing, or adding
/// one is neither breaking nor widening, and a public item that became internal is removed.
#[test]
fn internal_items_are_not_api_surface() {
    let internal = |path: &str, file: &str| ApiItem {
        internal: true,
        is_abstract: false,
        ..item(path, ApiItemKind::Function, "()", file, 1)
    };
    let base = vec![
        internal("tools.a.validate", "tools/a.x"),
        internal("tools.a.dropped", "tools/a.x"),
        function("m.hidden", "()", 3),
    ];
    let head = vec![
        internal("tools.b.validate", "tools/b.x"),
        internal("tools.b.fresh", "tools/b.x"),
        internal("m.hidden", "lib.x"),
    ];
    let (result, findings) = run(base, head);
    assert_eq!(
        reported(&findings),
        [("api.breaking", "m.hidden", "removed")]
    );
    assert_eq!(result.score, Some(score(1.0, "breaking", 1)));
}

/// A test file is no package's API: items defined in one are not compared on either side, so a
/// test helper whose signature changed, a removed one, and an added one report nothing.
#[test]
fn items_in_test_files_are_not_api_surface() {
    let in_test = |path: &str, signature: &str| {
        item(
            path,
            ApiItemKind::Function,
            signature,
            "test/ring/session.clj",
            30,
        )
    };
    let base = vec![
        in_test("ring.session-test/get-session-cookie", "[_]"),
        in_test("ring.session-test/gone", "[]"),
        function("ring.session/wrap", "[_]", 1),
    ];
    let head = vec![
        in_test("ring.session-test/get-session-cookie", "[_ _]"),
        in_test("ring.session-test/fresh", "[]"),
        function("ring.session/wrap", "[_ _]", 1),
    ];
    let (result, findings) = run(base, head);
    assert_eq!(
        reported(&findings),
        [("api.breaking", "ring.session/wrap", "changed")]
    );
    assert_eq!(result.score, Some(score(1.0, "breaking", 1)));
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
    support::assert_refuses_settings(ID);
}

#[test]
fn breaking_and_widened_add_up_across_languages() {
    support::assert_counts_add_up(ID, &["breaking", "widened"], "breaking");
}

#[test]
fn the_manifest_needs_api_surface_on_both_sides() {
    let check = support::check(ID);
    assert_eq!(check.base_needs(), [FactKind::ApiSurface]);
    assert_eq!(
        Shape::of(check.manifest()),
        Shape {
            needs: vec![FactKind::ApiSurface],
            wants: vec![FactKind::ModuleGraph, FactKind::TestConventions],
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
