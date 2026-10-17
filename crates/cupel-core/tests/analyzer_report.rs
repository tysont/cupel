// ABOUTME: Tests for the `AnalyzerReport` payload (ADR-102): its fact kind and versioned schema, its shape as
// ABOUTME: JSON, the status and level spellings, and the kind templates that match `analyze.<tool>.<rule>`.
mod support;

use cupel_core::{
    ActionKind, AnalyzerLevel, AnalyzerReport, AnalyzerResult, AnalyzerRun, AnalyzerStatus,
    FactKind, FindingKindSpec, Location, ToolId,
};
use serde_json::{Map, json};

#[test]
fn analyzer_report_is_an_execution_fact_with_a_versioned_schema() {
    assert_eq!(
        support::fact_entry(FactKind::AnalyzerReport),
        (
            "analyzer-report",
            false,
            Some(1),
            Some(json!(
                "https://cupel.dev/schema/v1/fact/analyzer-report.json"
            )),
        )
    );
}

#[test]
fn analyzer_report_serializes_every_field() {
    let report = AnalyzerReport {
        analyzers: vec![
            AnalyzerRun {
                tool: ToolId::from("mypy"),
                status: AnalyzerStatus::Ran,
                detail: None,
                results: vec![AnalyzerResult {
                    rule: "arg-type".to_owned(),
                    level: AnalyzerLevel::Error,
                    message: "Argument 2 has incompatible type".to_owned(),
                    location: Location {
                        file: "ledger.py".to_owned(),
                        line: 9,
                        col: Some(19),
                        end_line: 9,
                        end_col: Some(22),
                    },
                }],
            },
            AnalyzerRun {
                tool: ToolId::from("ruff"),
                status: AnalyzerStatus::Missing,
                detail: Some("python -m pip install ruff".to_owned()),
                results: Vec::new(),
            },
        ],
    };
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(
        value,
        json!({"analyzers": [
            {"tool": "mypy", "status": "ran", "detail": null, "results": [{
                "rule": "arg-type", "level": "error",
                "message": "Argument 2 has incompatible type",
                "location": {"file": "ledger.py", "line": 9, "col": 19, "end_line": 9, "end_col": 22},
            }]},
            {"tool": "ruff", "status": "missing", "detail": "python -m pip install ruff", "results": []},
        ]})
    );
    assert_eq!(
        serde_json::from_value::<AnalyzerReport>(value).unwrap(),
        report
    );
}

#[test]
fn statuses_and_levels_are_spelled_in_kebab_case() {
    let statuses = [
        AnalyzerStatus::Ran,
        AnalyzerStatus::Missing,
        AnalyzerStatus::Unconfigured,
        AnalyzerStatus::Failed,
    ]
    .map(|status| serde_json::to_value(status).unwrap());
    let levels = [
        AnalyzerLevel::Error,
        AnalyzerLevel::Warning,
        AnalyzerLevel::Note,
    ]
    .map(|level| serde_json::to_value(level).unwrap());
    assert_eq!(
        (statuses, levels),
        (
            ["ran", "missing", "unconfigured", "failed"].map(|s| json!(s)),
            ["error", "warning", "note"].map(|s| json!(s))
        )
    );
}

fn spec(kind: &str) -> FindingKindSpec {
    FindingKindSpec {
        kind: kind.to_owned(),
        action: ActionKind::FixBug,
        summary: String::new(),
        evidence: Map::new(),
    }
}

#[test]
fn a_kind_with_a_placeholder_segment_is_a_template() {
    let templates: Vec<bool> = ["analyze.<tool>.<rule>", "a.<b>.c", "api.breaking", "x.<y"]
        .iter()
        .map(|kind| spec(kind).is_template())
        .collect();
    assert_eq!(templates, [true, true, false, false]);
}

#[test]
fn a_kind_template_matches_one_segment_per_placeholder_and_the_rest_for_the_last() {
    let matched = |spec_kind: &str, kinds: &[&str]| -> Vec<bool> {
        let spec = spec(spec_kind);
        kinds.iter().map(|kind| spec.matches(kind)).collect()
    };
    assert_eq!(
        matched(
            "analyze.<tool>.<rule>",
            &[
                "analyze.ruff.F401",
                "analyze.go-vet.printf",
                "analyze.eslint.react.no-danger",
                "analyze.<tool>.<rule>",
                "analyze.ruff",
                "analyze",
                "lint.ruff.F401",
                "analyze..F401",
            ]
        ),
        [true, true, true, true, false, false, false, false]
    );
    assert_eq!(matched("a.<b>.c", &["a.x.c", "a.x.y.c"]), [true, false]);
    assert_eq!(
        matched("api.breaking", &["api.breaking", "api.breaking.more"]),
        [true, false]
    );
}
