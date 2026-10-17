// ABOUTME: Tests for the `AnalyzerReport` payload (ADR-102): its fact kind and versioned schema, its shape as
// ABOUTME: JSON, the status and level spellings, and the kind templates that match `analyze.<tool>.<rule>`.
use cupel_core::{
    ActionKind, AnalyzerLevel, AnalyzerReport, AnalyzerResult, AnalyzerRun, AnalyzerStatus,
    FactKind, FindingKindSpec, Location, ToolId, schema,
};
use serde_json::{Map, json};

#[test]
fn analyzer_report_is_an_execution_fact_with_a_versioned_schema() {
    let kind = FactKind::AnalyzerReport;
    assert_eq!(
        (
            kind.as_str(),
            FactKind::SYNTACTIC.contains(&kind),
            schema::fact_version(kind),
            schema::fact(kind).map(|schema| schema.as_value()["$id"].clone()),
        ),
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
    let spelled: Vec<_> = [
        AnalyzerStatus::Ran,
        AnalyzerStatus::Missing,
        AnalyzerStatus::Unconfigured,
        AnalyzerStatus::Failed,
    ]
    .iter()
    .map(|status| serde_json::to_value(status).unwrap())
    .chain(
        [
            AnalyzerLevel::Error,
            AnalyzerLevel::Warning,
            AnalyzerLevel::Note,
        ]
        .iter()
        .map(|level| serde_json::to_value(level).unwrap()),
    )
    .collect();
    assert_eq!(
        spelled,
        [
            "ran",
            "missing",
            "unconfigured",
            "failed",
            "error",
            "warning",
            "note"
        ]
        .map(|s| json!(s))
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
fn a_kind_template_matches_one_segment_per_placeholder_and_the_rest_for_the_last() {
    let template = spec("analyze.<tool>.<rule>");
    for kind in [
        "analyze.ruff.F401",
        "analyze.go-vet.printf",
        "analyze.eslint.react.no-danger",
        "analyze.<tool>.<rule>",
    ] {
        assert!(template.matches(kind), "{kind}");
    }
    for kind in ["analyze.ruff", "analyze", "lint.ruff.F401", "analyze..F401"] {
        assert!(!template.matches(kind), "{kind}");
    }
    let middle = spec("a.<b>.c");
    assert!(middle.matches("a.x.c"));
    assert!(!middle.matches("a.x.y.c"));
    let exact = spec("api.breaking");
    assert!(exact.matches("api.breaking"));
    assert!(!exact.matches("api.breaking.more"));
}
