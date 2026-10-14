// ABOUTME: Tests for `next_actions` (ADR-061): ranking by severity, novelty, and tier; de-duplication by
// ABOUTME: action kind and location; rerun, install, and configure actions; suppressed findings; the cap.
use cupel_core::{ActionKind, Envelope, NEXT_ACTIONS, NextAction, next_actions};
use serde_json::{Value, json};

struct F {
    id: &'static str,
    severity: &'static str,
    tier: u8,
    new: bool,
    action: &'static str,
    file: &'static str,
    line: u32,
}

const F0: F = F {
    id: "fnd_000000",
    severity: "warn",
    tier: 0,
    new: false,
    action: "review",
    file: "a.rs",
    line: 1,
};

fn finding(f: &F) -> Value {
    json!({
        "id": f.id, "check": "static.complexity", "kind": format!("kind.{}", f.id),
        "severity": f.severity, "confidence": 1.0, "tier": f.tier,
        "symbol": null,
        "location": {"file": f.file, "line": f.line, "col": null, "end_line": f.line + 2, "end_col": null},
        "title": "t", "detail": null, "evidence": {},
        "action": {"kind": f.action, "priority": 1, "summary": format!("act on {}", f.id), "hints": {}},
        "repro": "cupel check", "suppress": "cupel suppress", "new_since_baseline": f.new,
        "suppressed": false
    })
}

fn skipped(id: &str, reason: &str, lang: &str, tool: Option<&str>) -> Value {
    json!({
        "id": id, "status": "skipped", "tier": 0, "cost": "quick", "score": null, "metrics": {},
        "delta": null, "findings": 0, "duration_ms": 0,
        "skipped": {"reason": reason, "fact": null, "lang": lang, "tool": tool, "see": null},
        "oracle": null
    })
}

fn envelope(findings: &[Value], checks: &[Value], gate: &Value) -> Envelope {
    serde_json::from_value(json!({
        "cupel": {"version": "0.1.0", "schema": "https://cupel.dev/schema/v1/run.json"},
        "run": {
            "id": "run_01JA2B3C4D5E6F7G8H9JKMNPQA", "verb": "check",
            "started": "2026-10-14T10:00:00Z", "duration_ms": 1, "git": null, "scope": null,
            "depth": "standard", "budget_ms": null, "seed": 1, "toolchain": {},
            "config": {"effective": {}, "sources": []}, "baseline": null
        },
        "status": "pass", "exit_code": 0, "checks": checks,
        "total_findings": findings.len(), "findings": findings,
        "summary": {"findings": {"fail": 0, "warn": 0, "info": 0}, "new": 0, "resolved": 0,
                    "suppressed": 0, "gate": gate},
        "next_actions": [], "truncated": false
    }))
    .unwrap()
}

fn of(findings: &[F]) -> Vec<NextAction> {
    let findings: Vec<Value> = findings.iter().map(finding).collect();
    next_actions(&envelope(&findings, &[], &Value::Null), "cupel check")
}

fn ids(actions: &[NextAction]) -> Vec<String> {
    actions
        .iter()
        .map(|action| {
            action
                .finding
                .as_ref()
                .map_or_else(|| format!("{:?}", action.kind), ToString::to_string)
        })
        .collect()
}

#[test]
fn the_first_action_is_the_highest_severity_new_finding() {
    let actions = of(&[
        F {
            id: "fnd_000001",
            severity: "warn",
            tier: 3,
            new: true,
            line: 1,
            ..F0
        },
        F {
            id: "fnd_000002",
            severity: "fail",
            tier: 0,
            new: false,
            line: 10,
            ..F0
        },
        F {
            id: "fnd_000003",
            severity: "fail",
            tier: 0,
            new: true,
            line: 20,
            ..F0
        },
        F {
            id: "fnd_000004",
            severity: "info",
            tier: 6,
            new: true,
            line: 30,
            ..F0
        },
    ]);
    assert_eq!(actions[0].finding.as_ref().unwrap().as_str(), "fnd_000003");
    assert_eq!(
        ids(&actions),
        ["fnd_000003", "fnd_000002", "fnd_000001", "fnd_000004"]
    );
    let priorities: Vec<u32> = actions.iter().map(|action| action.priority).collect();
    assert_eq!(priorities, [1, 2, 3, 4]);
}

#[test]
fn within_a_severity_new_findings_come_before_higher_tiers_and_tiers_before_order() {
    let actions = of(&[
        F {
            id: "fnd_000001",
            tier: 0,
            new: false,
            line: 1,
            ..F0
        },
        F {
            id: "fnd_000002",
            tier: 3,
            new: false,
            line: 10,
            ..F0
        },
        F {
            id: "fnd_000003",
            tier: 0,
            new: true,
            line: 20,
            ..F0
        },
    ]);
    assert_eq!(ids(&actions), ["fnd_000003", "fnd_000002", "fnd_000001"]);
}

#[test]
fn findings_with_the_same_action_at_the_same_location_give_one_action() {
    let actions = of(&[
        F {
            id: "fnd_000001",
            action: "reduce_complexity",
            ..F0
        },
        F {
            id: "fnd_000002",
            action: "reduce_complexity",
            severity: "fail",
            ..F0
        },
        F {
            id: "fnd_000003",
            action: "refactor_extract",
            ..F0
        },
        F {
            id: "fnd_000004",
            action: "reduce_complexity",
            file: "b.rs",
            ..F0
        },
    ]);
    assert_eq!(ids(&actions), ["fnd_000002", "fnd_000003", "fnd_000004"]);
}

#[test]
fn suppressed_findings_get_no_action() {
    let mut suppressed = finding(&F {
        id: "fnd_000001",
        ..F0
    });
    suppressed["suppressed"] = json!(true);
    suppressed["severity"] = json!("info");
    let actions = next_actions(
        &envelope(
            &[
                suppressed,
                finding(&F {
                    id: "fnd_000002",
                    line: 9,
                    ..F0
                }),
            ],
            &[],
            &Value::Null,
        ),
        "cupel check",
    );
    assert_eq!(ids(&actions), ["fnd_000002"]);
}

#[test]
fn skipped_checks_get_rerun_install_and_configure_actions_and_unsupported_ones_none() {
    let checks = [
        skipped("static.slow", "timeout", "go", None),
        skipped("static.slow", "timeout", "rust", None),
        skipped(
            "tests.coverage",
            "unavailable",
            "python",
            Some("coverage.py"),
        ),
        skipped("tests.flaky", "unconfigured", "rust", None),
        skipped("static.deps", "unsupported", "go", None),
    ];
    let actions = next_actions(
        &envelope(&[], &checks, &Value::Null),
        "cupel check --changed",
    );
    let summary: Vec<(ActionKind, Option<&str>)> = actions
        .iter()
        .map(|action| (action.kind, action.command.as_deref()))
        .collect();
    assert_eq!(
        summary,
        [
            (
                ActionKind::Rerun,
                Some("cupel check --changed --only static.slow")
            ),
            (ActionKind::InstallTool, Some("cupel doctor")),
            (ActionKind::Configure, Some("cupel doctor")),
        ]
    );
    assert!(
        actions[1].summary.contains("`coverage.py`"),
        "{}",
        actions[1].summary
    );
    assert!(
        actions[1].summary.contains("python"),
        "{}",
        actions[1].summary
    );
    assert!(actions.iter().all(|action| action.finding.is_none()));
}

#[test]
fn gate_rules_that_could_not_pass_rank_after_fail_findings_and_before_warnings() {
    let gate = json!({
        "result": "fail", "evaluated": "gate.changed",
        "rules": [
            {"rule": "static.crap.max < 30", "actual": 12.0, "result": "pass"},
            {"rule": "tests.weakening none", "actual": null, "result": "skipped"},
            {"rule": "static.api-surface no-breaking", "actual": null, "result": "unsupported"},
            {"rule": "static.complexity.cyclomatic_max < 5", "actual": 11.0, "result": "fail"}
        ]
    });
    let findings = [
        finding(&F {
            id: "fnd_000001",
            severity: "warn",
            line: 1,
            ..F0
        }),
        finding(&F {
            id: "fnd_000002",
            severity: "fail",
            line: 10,
            ..F0
        }),
    ];
    let checks = [skipped("tests.weakening", "timeout", "rust", None)];
    let actions = next_actions(
        &envelope(&findings, &checks, &gate),
        "cupel check --changed",
    );
    assert_eq!(
        ids(&actions),
        ["fnd_000002", "Rerun", "Configure", "fnd_000001"],
        "the timeout's rerun of tests.weakening is the gate's rerun"
    );
    assert_eq!(
        actions[1].command.as_deref(),
        Some("cupel check --changed --only tests.weakening")
    );
    assert!(
        actions[2]
            .summary
            .contains("static.api-surface no-breaking")
    );
    assert_eq!(actions[2].command, None);
}

#[test]
fn at_most_ten_actions_are_listed() {
    let many: Vec<Value> = (0..15)
        .map(|n| {
            let mut value = finding(&F { line: n, ..F0 });
            value["id"] = json!(format!("fnd_0000{n:02}"));
            value
        })
        .collect();
    let actions = next_actions(&envelope(&many, &[], &Value::Null), "cupel check");
    assert_eq!(actions.len(), NEXT_ACTIONS);
    assert_eq!(NEXT_ACTIONS, 10);
}
