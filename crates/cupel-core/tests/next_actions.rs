// ABOUTME: Tests for `next_actions` (ADR-061, ADR-132): ranking by severity, novelty, and tier; de-duplication by
// ABOUTME: kind and location; rerun, install, and configure actions; suppressed findings; the cap; protect-first.
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
        "cupel": {"version": "0.1.0", "schema": "https://tysont.github.io/cupel/schema/v1/run.json"},
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
fn a_check_that_needs_a_range_gets_one_rerun_over_the_changed_files() {
    let needs_range = |lang: &str| {
        let mut check = skipped("tests.weakening", "needs-range", lang, None);
        check["skipped"]["see"] = json!("cupel check --changed --only tests.weakening");
        check
    };
    let checks = [needs_range("go"), needs_range("rust")];
    let actions = next_actions(&envelope(&[], &checks, &Value::Null), "cupel survey");
    assert_eq!(actions.len(), 1, "{actions:?}");
    assert_eq!(actions[0].kind, ActionKind::Rerun);
    assert_eq!(
        actions[0].command.as_deref(),
        Some("cupel check --changed --only tests.weakening")
    );
    assert!(
        actions[0].summary.contains("`tests.weakening`"),
        "{}",
        actions[0].summary
    );
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
fn a_skipped_rule_whose_check_needs_a_range_reruns_it_over_the_changed_files() {
    let gate = json!({
        "result": "fail", "evaluated": "gate.tree",
        "rules": [{"rule": "tests.weakening none", "actual": null, "result": "skipped"}]
    });
    let mut check = skipped("tests.weakening", "needs-range", "python", None);
    check["skipped"]["see"] = json!("cupel check --changed --only tests.weakening");
    let actions = next_actions(&envelope(&[], &[check], &gate), "cupel check");
    assert_eq!(
        ids(&actions),
        ["Rerun"],
        "one rerun for the rule and the skip"
    );
    assert_eq!(
        actions[0].command.as_deref(),
        Some("cupel check --changed --only tests.weakening"),
        "rerunning the tree scope would skip it again"
    );
    assert!(
        actions[0].summary.contains("tests.weakening none"),
        "{}",
        actions[0].summary
    );
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

/// A finding about the symbol `display` whose action is `action`.
fn symbol_finding(id: &str, display: &str, severity: &str, action: &str) -> Value {
    let mut finding = finding(&F0);
    finding["id"] = json!(id);
    finding["severity"] = json!(severity);
    finding["action"]["kind"] = json!(action);
    finding["action"]["summary"] = json!(format!("act on {id}"));
    finding["symbol"] = json!({"display": display, "hash": "sh_00000000000000aa", "lang": "rust"});
    finding["location"]["file"] = json!(format!("{display}.rs"));
    finding
}

/// A survey's `rankings` whose targets are `targets`, each `(display, recommend, findings)`.
fn rankings(targets: &[(&str, &str, &[&str])]) -> cupel_core::Rankings {
    let targets: Vec<Value> = targets
        .iter()
        .zip(1..)
        .map(|((display, recommend, findings), rank)| {
            json!({
                "rank": rank, "recommend": recommend, "lang": "rust",
                "target": {"display": display, "symbol": "sh_00000000000000aa", "location": null},
                "protection": {"mutation_score": null, "coverage": null, "tests": null},
                "values": [], "findings": findings
            })
        })
        .collect();
    serde_json::from_value(json!({
        "top": 10, "dimensions": [], "recommended": targets.len(), "targets": targets
    }))
    .unwrap()
}

fn ids_of(ids: &[&str]) -> Vec<String> {
    ids.iter().map(ToString::to_string).collect()
}

#[test]
fn a_survey_lists_every_target_protect_action_before_any_simplify_action() {
    let findings = [
        symbol_finding("fnd_00000c", "c", "fail", "fix_bug"),
        symbol_finding("fnd_0000a1", "a", "warn", "reduce_complexity"),
        symbol_finding("fnd_0000b2", "b", "warn", "refactor_extract"),
        symbol_finding("fnd_0000a2", "a", "info", "strengthen_assertion"),
        symbol_finding("fnd_0000b1", "b", "info", "write_test"),
    ];
    let mut survey = envelope(&findings, &[], &Value::Null);
    let before = ids(&next_actions(&survey, "cupel survey"));
    survey.rankings = Some(rankings(&[
        ("a", "both", &["fnd_0000a1", "fnd_0000a2"]),
        ("b", "both", &["fnd_0000b2", "fnd_0000b1"]),
    ]));
    let actions = next_actions(&survey, "cupel survey");
    let kinds: Vec<ActionKind> = actions.iter().map(|action| action.kind).collect();
    assert_eq!(
        (before, ids(&actions), kinds),
        (
            ids_of(&[
                "fnd_00000c",
                "fnd_0000a1",
                "fnd_0000b2",
                "fnd_0000a2",
                "fnd_0000b1"
            ]),
            ids_of(&[
                "fnd_0000a2",
                "fnd_0000b1",
                "fnd_0000a1",
                "fnd_0000b2",
                "fnd_00000c"
            ]),
            vec![
                ActionKind::StrengthenAssertion,
                ActionKind::WriteTest,
                ActionKind::ReduceComplexity,
                ActionKind::RefactorExtract,
                ActionKind::FixBug,
            ]
        )
    );
}

#[test]
fn a_target_whose_finding_is_suppressed_gets_no_action_for_it() {
    let mut suppressed = symbol_finding("fnd_0000a2", "a", "info", "write_test");
    suppressed["suppressed"] = json!(true);
    let findings = [
        symbol_finding("fnd_0000a1", "a", "warn", "reduce_complexity"),
        suppressed,
    ];
    let mut survey = envelope(&findings, &[], &Value::Null);
    survey.rankings = Some(rankings(&[("a", "both", &["fnd_0000a1", "fnd_0000a2"])]));
    assert_eq!(
        ids(&next_actions(&survey, "cupel survey")),
        ids_of(&["fnd_0000a1"])
    );
}
