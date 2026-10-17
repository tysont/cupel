// ABOUTME: static.analyze over synthetic AnalyzerReport facts: one finding per result with its templated kind,
// ABOUTME: severities by level, identity across line moves, notes for analyzers that did not run, and the score (ADR-102).
mod support;

use cupel_core::{
    ActionKind, AnalyzerLevel, AnalyzerReport, AnalyzerResult, AnalyzerRun, AnalyzerStatus,
    CheckId, CheckResult, CheckStatus, CostClass, FactKind, Facts, Finding, FindingId, Location,
    MetricPer, Scope, Severity, ToolId,
};
use serde_json::json;
use support::{Shape, score};

const ID: &str = "static.analyze";

fn result(rule: &str, level: AnalyzerLevel, message: &str, line: u32) -> AnalyzerResult {
    AnalyzerResult {
        rule: rule.to_owned(),
        level,
        message: message.to_owned(),
        location: Location {
            file: "ledger.py".to_owned(),
            line,
            col: Some(5),
            end_line: line,
            end_col: Some(9),
        },
    }
}

fn ran(tool: &str, results: Vec<AnalyzerResult>) -> AnalyzerRun {
    AnalyzerRun {
        tool: ToolId::from(tool),
        status: AnalyzerStatus::Ran,
        detail: None,
        results,
    }
}

fn not_run(tool: &str, status: AnalyzerStatus, detail: &str) -> AnalyzerRun {
    AnalyzerRun {
        tool: ToolId::from(tool),
        status,
        detail: Some(detail.to_owned()),
        results: Vec::new(),
    }
}

fn run(analyzers: Vec<AnalyzerRun>) -> (CheckResult, Vec<Finding>) {
    let check = support::check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(AnalyzerReport { analyzers }).unwrap();
    let scope = Scope::of_files(vec!["ledger.py".to_owned()]);
    let (result, recorded) = support::recorded(check, &facts, scope, json!({})).unwrap();
    (result, recorded.findings)
}

#[test]
fn each_result_is_a_finding_of_its_tool_and_rule_at_its_location() {
    let (result, findings) = run(vec![
        ran(
            "mypy",
            vec![result("arg-type", AnalyzerLevel::Error, "Bad argument", 9)],
        ),
        ran(
            "ruff",
            vec![result("F401", AnalyzerLevel::Warning, "`os` unused", 1)],
        ),
    ]);
    let kinds: Vec<(&str, Severity)> = findings
        .iter()
        .map(|finding| (finding.kind.as_str(), finding.severity))
        .collect();
    assert_eq!(
        kinds,
        [
            ("analyze.mypy.arg-type", Severity::Fail),
            ("analyze.ruff.F401", Severity::Warn)
        ]
    );
    let first = &findings[0];
    assert_eq!(
        first.evidence.0,
        *json!({"tool": "mypy", "rule": "arg-type", "level": "error", "message": "Bad argument"})
            .as_object()
            .unwrap()
    );
    assert_eq!(first.title, "mypy arg-type: Bad argument");
    assert_eq!(first.action.kind, ActionKind::FixBug);
    let at = first.location.as_ref().unwrap();
    assert_eq!(
        (at.file.as_str(), at.line, at.col),
        ("ledger.py", 9, Some(5))
    );
    assert_eq!(
        first.repro,
        "cupel check --only static.analyze --paths ledger.py"
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(
        result.metrics,
        [("diagnostics".to_owned(), 2.0), ("errors".to_owned(), 1.0)].into()
    );
    assert_eq!(result.score, Some(score(2.0, "diagnostics", 2)));
}

#[test]
fn a_warning_fails_the_check_and_notes_alone_pass_as_info() {
    let (warned, findings) = run(vec![ran(
        "go-vet",
        vec![
            result("printf", AnalyzerLevel::Warning, "bad format", 6),
            result("misc", AnalyzerLevel::Note, "see here", 7),
        ],
    )]);
    assert_eq!(
        findings.iter().map(|f| f.severity).collect::<Vec<_>>(),
        [Severity::Warn, Severity::Info]
    );
    assert_eq!(warned.status, CheckStatus::Fail);
    assert_eq!(warned.metrics.get("errors"), Some(&0.0));
    let (noted, _) = run(vec![ran(
        "mypy",
        vec![result("misc", AnalyzerLevel::Note, "see here", 7)],
    )]);
    assert_eq!(noted.status, CheckStatus::Pass);
}

#[test]
fn a_finding_keeps_its_id_when_its_line_moves_and_repeats_are_told_apart() {
    let id_at = |line| {
        let (_, findings) = run(vec![ran(
            "ruff",
            vec![result("F401", AnalyzerLevel::Warning, "`os` unused", line)],
        )]);
        findings[0].id.clone()
    };
    assert_eq!(id_at(1), id_at(30));
    let check = CheckId::from(ID);
    assert_eq!(
        id_at(1),
        FindingId::for_subject(&check, "analyze.ruff.F401", "ledger.py|`os` unused|0")
    );
    let (_, twice) = run(vec![ran(
        "ruff",
        vec![
            result("F401", AnalyzerLevel::Warning, "`os` unused", 1),
            result("F401", AnalyzerLevel::Warning, "`os` unused", 4),
        ],
    )]);
    assert_ne!(twice[0].id, twice[1].id);
}

#[test]
fn rules_become_kind_segments_the_manifest_template_matches() {
    let (_, findings) = run(vec![ran(
        "eslint",
        vec![
            result(
                "@typescript-eslint/no-explicit-any",
                AnalyzerLevel::Error,
                "No any",
                1,
            ),
            result("react.no-danger", AnalyzerLevel::Error, "Danger", 2),
        ],
    )]);
    let kinds: Vec<&str> = findings.iter().map(|f| f.kind.as_str()).collect();
    assert_eq!(
        kinds,
        [
            "analyze.eslint.typescript-eslint-no-explicit-any",
            "analyze.eslint.react.no-danger"
        ]
    );
    let template = &support::check(ID).manifest().findings[0];
    assert!(kinds.iter().all(|kind| template.matches(kind)));
}

#[test]
fn analyzers_that_did_not_run_are_notes_and_none_running_scores_nothing() {
    let (result, findings) = run(vec![
        not_run(
            "mypy",
            AnalyzerStatus::Missing,
            "python -m pip install mypy",
        ),
        not_run(
            "eslint",
            AnalyzerStatus::Unconfigured,
            "Add an ESLint flat config.",
        ),
    ]);
    assert_eq!(findings, []);
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.score, None);
    assert!(result.metrics.is_empty());
    assert_eq!(
        result.notes,
        [
            "mypy missing: python -m pip install mypy",
            "eslint unconfigured: Add an ESLint flat config."
        ]
    );
}

#[test]
fn a_failed_analyzer_is_a_note_beside_the_results_of_the_others() {
    let (result, findings) = run(vec![
        ran(
            "mypy",
            vec![result("arg-type", AnalyzerLevel::Error, "Bad", 9)],
        ),
        not_run("ruff", AnalyzerStatus::Failed, "ruff.toml: bad"),
    ]);
    assert_eq!(findings.len(), 1);
    assert_eq!(result.notes, ["ruff failed: ruff.toml: bad"]);
    assert_eq!(result.score, Some(score(1.0, "diagnostics", 1)));
}

#[test]
fn settings_are_refused() {
    support::assert_refuses_settings(ID);
}

#[test]
fn counts_add_up_across_languages() {
    support::assert_counts_add_up(ID, &["errors", "diagnostics"], "diagnostics");
}

#[test]
fn the_manifest_declares_one_templated_kind_over_analyzer_report() {
    assert_eq!(
        Shape::of(support::check(ID).manifest()),
        Shape {
            needs: vec![FactKind::AnalyzerReport],
            wants: vec![],
            tier: 1,
            cost: CostClass::Standard,
            kinds: vec![("analyze.<tool>.<rule>", ActionKind::FixBug)],
            metrics: vec![
                ("errors", MetricPer::Scope),
                ("diagnostics", MetricPer::Scope)
            ],
            evidence: vec!["object"],
        }
    );
}
