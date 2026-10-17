// ABOUTME: Tests for gate evaluation (ADR-060): every operator passing and failing, absent metrics and
// ABOUTME: checks, presence after suppression, `no-breaking`, table choice by scope mode, and status.
use cupel_core::config::{ConfigFile, GateRuleSpec};
use cupel_core::{
    Envelope, ExitCode, GateOutcome, GateResult, RuleOutcome, ScopeInfo, ScopeMode, Status,
    apply_gate, evaluate_gate, gate_for,
};
use serde_json::{Value, json};

fn finding(id: &str, check: &str, severity: &str, suppressed: bool) -> Value {
    json!({
        "id": id, "check": check, "kind": "k",
        "severity": severity, "confidence": 1.0, "tier": 0,
        "symbol": null, "location": null, "title": "t", "detail": null,
        "evidence": {}, "action": {"kind": "review", "priority": 1, "summary": "Look.", "hints": {}},
        "repro": "cupel check", "suppress": "cupel suppress", "new_since_baseline": false,
        "suppressed": suppressed
    })
}

fn check(id: &str, status: &str, metrics: &Value) -> Value {
    json!({
        "id": id, "status": status, "tier": 0, "cost": "quick", "score": null,
        "metrics": metrics, "delta": null, "findings": 0, "duration_ms": 1,
        "skipped": if status == "skipped" {
            json!({"reason": "unsupported", "fact": null, "lang": "go", "tool": null, "see": null})
        } else {
            Value::Null
        },
        "oracle": null
    })
}

fn envelope(status: &str, checks: &[Value], findings: &[Value]) -> Envelope {
    let exit_code = match status {
        "partial" => 5,
        "error" => 2,
        _ => 0,
    };
    serde_json::from_value(json!({
        "cupel": {"version": "0.1.0", "schema": "https://cupel.dev/schema/v1/run.json"},
        "run": {
            "id": "run_01JA2B3C4D5E6F7G8H9JKMNPQA", "verb": "check",
            "started": "2026-10-14T10:00:00Z", "duration_ms": 1, "git": null, "scope": null,
            "depth": "standard", "budget_ms": null, "seed": 1, "toolchain": {},
            "config": {"effective": {}, "sources": []}, "baseline": null
        },
        "status": status, "exit_code": exit_code, "checks": checks,
        "total_findings": findings.len(), "findings": findings,
        "summary": {"findings": {"fail": 0, "warn": 0, "info": 0}, "new": 0, "resolved": 0,
                    "suppressed": 0, "gate": null},
        "next_actions": [], "truncated": false
    }))
    .unwrap()
}

/// A run in which `static.crap` reported `max = 20` and `tests.mutation` reported `score = 0.75`.
fn measured() -> Envelope {
    envelope(
        "pass",
        &[
            check("static.crap", "pass", &json!({"max": 20.0})),
            check("tests.mutation", "fail", &json!({"score": 0.75})),
        ],
        &[],
    )
}

fn rule(key: &str, condition: &str) -> GateRuleSpec {
    GateRuleSpec::parse(key, condition).unwrap()
}

/// One rule's `(actual, result)` against `envelope`.
fn one(key: &str, condition: &str, envelope: &Envelope) -> (Option<f64>, RuleOutcome) {
    let gate = evaluate_gate("gate.tree", &[rule(key, condition)], envelope);
    (gate.rules[0].actual, gate.rules[0].result)
}

#[test]
fn every_comparison_operator_passes_and_fails() {
    let cases = [
        (">= 20", RuleOutcome::Pass),
        (">= 20.5", RuleOutcome::Fail),
        ("> 19", RuleOutcome::Pass),
        ("> 20", RuleOutcome::Fail),
        ("<= 20", RuleOutcome::Pass),
        ("<= 19.9", RuleOutcome::Fail),
        ("< 30", RuleOutcome::Pass),
        ("< 20", RuleOutcome::Fail),
        ("== 20", RuleOutcome::Pass),
        ("== 21", RuleOutcome::Fail),
    ];
    let envelope = measured();
    for (condition, expected) in cases {
        assert_eq!(
            one("static.crap.max", condition, &envelope),
            (Some(20.0), expected),
            "{condition}"
        );
    }
}

#[test]
fn a_percent_threshold_compares_with_the_ratio() {
    let envelope = measured();
    assert_eq!(
        one("tests.mutation.score", ">= 70%", &envelope),
        (Some(0.75), RuleOutcome::Pass)
    );
    assert_eq!(
        one("tests.mutation.score", ">= 80%", &envelope),
        (Some(0.75), RuleOutcome::Fail)
    );
}

#[test]
fn a_rule_whose_metric_or_check_is_absent_is_skipped_and_fails_the_gate() {
    let skipped = envelope(
        "pass",
        &[
            check("static.crap", "skipped", &json!({})),
            check("static.deps", "error", &json!({})),
            check("static.clones", "partial", &json!({"max": 1.0})),
            check("tests.mutation", "pass", &json!({})),
        ],
        &[],
    );
    for (key, condition) in [
        ("static.crap.max", "< 30"),
        ("static.deps.cycles", "== 0"),
        ("static.clones.max", "< 5"),
        ("tests.mutation.score", ">= 0.8"),
        ("tests.weakening", "none"),
        ("static.deps", "none"),
    ] {
        assert_eq!(
            one(key, condition, &skipped),
            (None, RuleOutcome::Skipped),
            "{key} {condition}"
        );
    }
    let gate = evaluate_gate("gate.tree", &[rule("tests.weakening", "none")], &skipped);
    assert_eq!(gate.result, GateOutcome::Fail);
}

#[test]
fn a_check_that_ran_for_one_language_and_was_skipped_for_another_is_evaluated() {
    let mixed = envelope(
        "pass",
        &[
            check("static.crap", "pass", &json!({"max": 4.0})),
            check("static.crap", "skipped", &json!({})),
        ],
        &[],
    );
    assert_eq!(
        one("static.crap.max", "< 30", &mixed),
        (Some(4.0), RuleOutcome::Pass)
    );
}

#[test]
fn none_counts_warn_and_fail_findings_after_suppression() {
    let checks = [check("tests.weakening", "pass", &json!({}))];
    let clean = envelope(
        "pass",
        &checks,
        &[
            finding("fnd_000001", "tests.weakening", "info", true),
            finding("fnd_000002", "tests.weakening", "info", false),
            finding("fnd_000003", "static.crap", "fail", false),
        ],
    );
    assert_eq!(
        one("tests.weakening", "none", &clean),
        (Some(0.0), RuleOutcome::Pass)
    );
    let weakened = envelope(
        "pass",
        &checks,
        &[
            finding("fnd_000001", "tests.weakening", "fail", false),
            finding("fnd_000002", "tests.weakening", "warn", false),
        ],
    );
    assert_eq!(
        one("tests.weakening", "none", &weakened),
        (Some(2.0), RuleOutcome::Fail)
    );
}

/// `finding` of `kind`.
fn of_kind(kind: &str, id: &str, check: &str, severity: &str) -> Value {
    let mut finding = finding(id, check, severity, false);
    finding["kind"] = json!(kind);
    finding
}

#[test]
fn no_breaking_counts_the_check_s_breaking_findings_above_info() {
    let checks = [check("static.api-surface", "fail", &json!({}))];
    let broke = envelope(
        "pass",
        &checks,
        &[
            of_kind("api.breaking", "fnd_000001", "static.api-surface", "warn"),
            of_kind("api.breaking", "fnd_000002", "static.api-surface", "warn"),
            of_kind("api.widened", "fnd_000003", "static.api-surface", "warn"),
            of_kind("api.breaking", "fnd_000004", "static.api-surface", "info"),
            of_kind("api.breaking", "fnd_000005", "static.crap", "warn"),
        ],
    );
    assert_eq!(
        one("static.api-surface", "no-breaking", &broke),
        (Some(2.0), RuleOutcome::Fail)
    );
    let widened = envelope(
        "pass",
        &checks,
        &[of_kind(
            "api.widened",
            "fnd_000003",
            "static.api-surface",
            "info",
        )],
    );
    assert_eq!(
        one("static.api-surface", "no-breaking", &widened),
        (Some(0.0), RuleOutcome::Pass)
    );
}

#[test]
fn no_breaking_is_skipped_when_its_check_did_not_run() {
    let envelope = envelope(
        "pass",
        &[check("static.api-surface", "skipped", &json!({}))],
        &[],
    );
    assert_eq!(
        one("static.api-surface", "no-breaking", &envelope),
        (None, RuleOutcome::Skipped)
    );
}

#[test]
fn the_gate_passes_only_when_every_rule_passes_and_reports_rules_as_written() {
    let envelope = measured();
    let gate = evaluate_gate(
        "gate.changed",
        &[
            rule("static.crap.max", "< 30"),
            rule("tests.mutation.score", ">= 0.80"),
        ],
        &envelope,
    );
    assert_eq!(gate.evaluated, "gate.changed");
    assert_eq!(gate.result, GateOutcome::Fail);
    let rules: Vec<(&str, RuleOutcome)> = gate
        .rules
        .iter()
        .map(|rule| (rule.rule.as_str(), rule.result))
        .collect();
    assert_eq!(
        rules,
        [
            ("static.crap.max < 30", RuleOutcome::Pass),
            ("tests.mutation.score >= 0.8", RuleOutcome::Fail),
        ]
    );
    let empty = evaluate_gate("gate.tree", &[], &envelope);
    assert_eq!(empty.result, GateOutcome::Pass);
}

#[test]
fn changed_and_range_scopes_use_gate_changed_and_the_rest_gate_tree() {
    let file = ConfigFile::parse(
        "[gate.changed]\n\"static.crap.max\" = \"< 30\"\n",
        std::path::Path::new("/repo/cupel.toml"),
    )
    .unwrap();
    let pick = |mode| {
        let (name, rules) = gate_for(&file.gate, mode);
        (name, rules.map(|section| section.rules().len()))
    };
    assert_eq!(pick(ScopeMode::Changed), ("gate.changed", Some(1)));
    assert_eq!(pick(ScopeMode::Range), ("gate.changed", Some(1)));
    assert_eq!(pick(ScopeMode::SinceBaseline), ("gate.changed", Some(1)));
    for mode in [ScopeMode::Tree, ScopeMode::Paths, ScopeMode::Symbols] {
        assert_eq!(pick(mode), ("gate.tree", None), "{mode:?}");
    }
}

fn result(outcome: GateOutcome) -> GateResult {
    GateResult {
        result: outcome,
        evaluated: "gate.tree".to_owned(),
        rules: Vec::new(),
    }
}

#[test]
fn a_failed_gate_fails_a_passing_run_with_exit_1_and_leaves_partial_and_error_alone() {
    let mut passing = envelope("pass", &[], &[]);
    apply_gate(&mut passing, result(GateOutcome::Pass));
    assert_eq!(
        (passing.status, passing.exit_code),
        (Status::Pass, ExitCode::Pass)
    );
    assert!(passing.summary.gate.is_some());

    let mut failing = envelope("pass", &[], &[]);
    apply_gate(&mut failing, result(GateOutcome::Fail));
    assert_eq!(
        (failing.status, failing.exit_code),
        (Status::Fail, ExitCode::Fail)
    );

    for (status, exit_code) in [("partial", ExitCode::Partial), ("error", ExitCode::Usage)] {
        let mut envelope = envelope(status, &[], &[]);
        let before = envelope.status;
        apply_gate(&mut envelope, result(GateOutcome::Fail));
        assert_eq!((envelope.status, envelope.exit_code), (before, exit_code));
        assert_eq!(
            envelope.summary.gate.as_ref().map(|gate| gate.result),
            Some(GateOutcome::Fail)
        );
    }
}

#[test]
fn an_empty_scope_passes_every_rule_vacuously() {
    let mut empty = envelope("pass", &[check("static.crap", "pass", &json!({}))], &[]);
    empty.run.scope = Some(ScopeInfo {
        mode: ScopeMode::Changed,
        files: 0,
        symbols: 0,
        roots: vec![".".to_owned()],
        languages: vec![],
    });
    let gate = evaluate_gate(
        "gate.changed",
        &[
            rule("static.crap.max", "< 30"),
            rule("tests.weakening", "none"),
        ],
        &empty,
    );
    assert_eq!(gate.result, GateOutcome::Pass);
    assert!(gate.rules.iter().all(|rule| rule.actual.is_none()));
}
