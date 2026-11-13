// ABOUTME: Tests for gate evaluation (ADR-060): every operator passing and failing, absent metrics and
// ABOUTME: checks, presence after suppression, `no-breaking`, rules against a reference run, table choice, and status.
mod support;

use cupel_core::config::{ConfigFile, GateRuleSpec};
use cupel_core::{
    CheckRegistry, Envelope, ExitCode, GateOutcome, GateReference, GateResult, GateRule, LangId,
    MetricPer, MetricSpec, RuleOutcome, ScopeInfo, ScopeMode, SettingSpec, Status, Tolerances,
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
        "cupel": {"version": "0.1.0", "schema": "https://tysont.github.io/cupel/schema/v1/run.json"},
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

/// The reference of a run with no baseline and no range.
fn unset() -> GateReference<'static> {
    GateReference::Missing("no baseline is set".to_owned())
}

/// One rule's result against `envelope`, with `reference` as `base` and no metric tolerances.
fn against(key: &str, condition: &str, envelope: &Envelope, reference: &GateReference) -> GateRule {
    tolerated(key, condition, envelope, reference, &Tolerances::default())
}

/// One rule's result against `envelope`, with `reference` as `base` and `tolerances` declared.
fn tolerated(
    key: &str,
    condition: &str,
    envelope: &Envelope,
    reference: &GateReference,
    tolerances: &Tolerances,
) -> GateRule {
    let gate = evaluate_gate(
        "gate.tree",
        &[rule(key, condition)],
        envelope,
        reference,
        tolerances,
    );
    gate.rules.into_iter().next().unwrap()
}

/// One rule's `(actual, result)` against `envelope`.
fn one(key: &str, condition: &str, envelope: &Envelope) -> (Option<f64>, RuleOutcome) {
    let rule = against(key, condition, envelope, &unset());
    (rule.actual, rule.result)
}

fn gate_rule(
    rule: &str,
    actual: Option<f64>,
    bound: Option<f64>,
    result: RuleOutcome,
    reason: Option<&str>,
) -> GateRule {
    GateRule {
        rule: rule.to_owned(),
        actual,
        actual_target: None,
        bound,
        bound_target: None,
        tolerance: None,
        watch: None,
        result,
        reason: reason.map(str::to_owned),
    }
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
    let gate = evaluate_gate(
        "gate.tree",
        &[rule("tests.weakening", "none")],
        &skipped,
        &unset(),
        &Tolerances::default(),
    );
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
        &unset(),
        &Tolerances::default(),
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
    let empty = evaluate_gate(
        "gate.tree",
        &[],
        &envelope,
        &unset(),
        &Tolerances::default(),
    );
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
            rule("static.crap.max", "<= base"),
        ],
        &empty,
        &unset(),
        &Tolerances::default(),
    );
    assert_eq!(gate.result, GateOutcome::Pass);
    assert!(gate.rules.iter().all(|rule| rule.actual.is_none()));
}

#[test]
fn a_skipped_rule_says_why_and_an_evaluated_one_carries_its_bound() {
    let envelope = envelope(
        "pass",
        &[
            check("static.crap", "skipped", &json!({})),
            check("tests.mutation", "pass", &json!({"score": 0.75})),
        ],
        &[],
    );
    let rules = [
        against("static.crap.max", "< 30", &envelope, &unset()),
        against("tests.mutation.max", "< 30", &envelope, &unset()),
        against("tests.mutation.score", ">= 70%", &envelope, &unset()),
        against("tests.mutation", "none", &envelope, &unset()),
    ];
    assert_eq!(
        rules,
        [
            gate_rule(
                "static.crap.max < 30",
                None,
                None,
                RuleOutcome::Skipped,
                Some("`static.crap` was skipped (unsupported)"),
            ),
            gate_rule(
                "tests.mutation.max < 30",
                None,
                None,
                RuleOutcome::Skipped,
                Some("`tests.mutation` did not report `max`"),
            ),
            gate_rule(
                "tests.mutation.score >= 70%",
                Some(0.75),
                Some(0.7),
                RuleOutcome::Pass,
                None,
            ),
            gate_rule(
                "tests.mutation none",
                Some(0.0),
                Some(0.0),
                RuleOutcome::Pass,
                None,
            ),
        ]
    );
}

#[test]
fn a_rule_whose_check_did_not_complete_says_whether_it_was_unselected_skipped_or_cut_short() {
    let envelope = envelope(
        "pass",
        &[
            check("static.crap", "skipped", &json!({})),
            check("static.deps", "error", &json!({})),
        ],
        &[],
    );
    let reasons: Vec<Option<String>> = [
        "static.complexity.cyclomatic_max",
        "static.crap.max",
        "static.deps.cycles",
    ]
    .iter()
    .map(|key| against(key, "< 30", &envelope, &unset()).reason)
    .collect();
    assert_eq!(
        reasons,
        [
            Some("`static.complexity` was not selected for this run".to_owned()),
            Some("`static.crap` was skipped (unsupported)".to_owned()),
            Some("`static.deps` did not run to completion".to_owned()),
        ]
    );
}

#[test]
fn a_rule_against_base_compares_with_the_reference_value_plus_its_offset() {
    let current = measured();
    let reference = envelope(
        "pass",
        &[check("static.crap", "pass", &json!({"max": 20.0}))],
        &[],
    );
    let base = GateReference::Run(&reference);
    let rules: Vec<GateRule> = [
        "<= base",
        "< base",
        ">= base + 1",
        "> base - 0.5",
        "== base",
    ]
    .into_iter()
    .map(|condition| against("static.crap.max", condition, &current, &base))
    .collect();
    let crap =
        |rule: &str, bound: f64, result| gate_rule(rule, Some(20.0), Some(bound), result, None);
    assert_eq!(
        rules,
        [
            with_tolerance(
                crap("static.crap.max <= base", 20.0, RuleOutcome::Pass),
                0.0
            ),
            with_tolerance(crap("static.crap.max < base", 20.0, RuleOutcome::Fail), 0.0),
            crap("static.crap.max >= base + 1", 21.0, RuleOutcome::Fail),
            crap("static.crap.max > base - 0.5", 19.5, RuleOutcome::Pass),
            crap("static.crap.max == base", 20.0, RuleOutcome::Pass),
        ]
    );
}

#[test]
fn a_rule_against_base_without_a_reference_value_is_skipped() {
    let current = measured();
    let unmeasured = envelope(
        "pass",
        &[
            check("static.crap", "pass", &json!({})),
            check("tests.mutation", "skipped", &json!({})),
        ],
        &[],
    );
    let ran = GateReference::Run(&unmeasured);
    let rules = [
        against("static.crap.max", "<= base", &current, &unset()),
        against("static.crap.max", "<= base", &current, &ran),
        against("tests.mutation.score", ">= base - 0.02", &current, &ran),
    ];
    assert_eq!(
        rules,
        [
            gate_rule(
                "static.crap.max <= base",
                Some(20.0),
                None,
                RuleOutcome::Skipped,
                Some("no reference: no baseline is set"),
            ),
            gate_rule(
                "static.crap.max <= base",
                Some(20.0),
                None,
                RuleOutcome::Skipped,
                Some("no reference: the reference run did not report `static.crap.max`"),
            ),
            gate_rule(
                "tests.mutation.score >= base - 0.02",
                Some(0.75),
                None,
                RuleOutcome::Skipped,
                Some("no reference: the reference run did not report `tests.mutation.score`"),
            ),
        ]
    );
}

#[test]
fn a_rule_without_a_reference_leaves_the_gate_to_the_other_rules() {
    // The rule lacking a reference is skipped and the others decide (ADR-198).
    let current = measured();
    let gate = |condition: &str| {
        evaluate_gate(
            "gate.tree",
            &[
                rule("static.crap.max", "<= base"),
                rule("tests.mutation.score", condition),
            ],
            &current,
            &unset(),
            &Tolerances::default(),
        )
        .result
    };
    assert_eq!(
        [gate(">= 0.7"), gate(">= 0.8")],
        [GateOutcome::Pass, GateOutcome::Fail]
    );
}

/// What a symbol in `file` at `line` is as a metric's target.
fn target(display: &str, file: &str, line: u32) -> Value {
    json!({
        "display": display, "symbol": null,
        "location": {"file": file, "line": line, "col": 1, "end_line": line + 9, "end_col": 2}
    })
}

/// A run in which `static.complexity` reported `cyclomatic_max` as `value`, measured on `on`.
fn complexity(value: f64, on: &Value) -> Envelope {
    let mut result = check(
        "static.complexity",
        "fail",
        &json!({"cyclomatic_max": value, "length_max": 40.0}),
    );
    result["metric_targets"] = json!({"cyclomatic_max": on});
    envelope("pass", &[result], &[])
}

#[test]
fn a_rule_names_the_targets_its_actual_and_its_reference_bound_were_measured_on() {
    let (head, base) = (
        target("a::worst", "src/a.rs", 3),
        target("b::was", "src/b.rs", 7),
    );
    let current = complexity(30.0, &head);
    let reference = complexity(18.0, &base);
    let against_base = against(
        "static.complexity.cyclomatic_max",
        "<= base",
        &current,
        &GateReference::Run(&reference),
    );
    let serialized = serde_json::to_value(&against_base).unwrap();
    assert_eq!(
        (&serialized["actual_target"], &serialized["bound_target"]),
        (&head, &base)
    );
    // A threshold is no target's value, and a metric that is no target's has none.
    let threshold = serde_json::to_value(against(
        "static.complexity.cyclomatic_max",
        "<= 18",
        &current,
        &unset(),
    ))
    .unwrap();
    assert_eq!(threshold["actual_target"], head);
    assert_eq!(threshold.get("bound_target"), None);
    let untargeted = serde_json::to_value(against(
        "static.complexity.length_max",
        "<= 60",
        &current,
        &unset(),
    ))
    .unwrap();
    assert_eq!(untargeted.get("actual_target"), None);
}

/// `gate_rule` for a rule against `base` whose bound applied the metric's `tolerance`.
fn with_tolerance(mut rule: GateRule, tolerance: f64) -> GateRule {
    rule.tolerance = Some(tolerance);
    rule
}

#[test]
fn a_rule_against_base_loosens_its_bound_by_the_metric_s_tolerance_unless_it_writes_an_offset() {
    let current = measured();
    let reference = envelope(
        "pass",
        &[
            check("static.crap", "pass", &json!({"max": 20.0})),
            check("tests.mutation", "pass", &json!({"score": 0.76})),
        ],
        &[],
    );
    let base = GateReference::Run(&reference);
    let tolerances = Tolerances::from_iter([("tests.mutation.score".to_owned(), 0.02)]);
    let rules: Vec<GateRule> = [
        ("tests.mutation.score", ">= base"),
        ("tests.mutation.score", "> base"),
        ("tests.mutation.score", ">= base - 0"),
        ("tests.mutation.score", "== base"),
        ("static.crap.max", "<= base"),
    ]
    .into_iter()
    .map(|(key, condition)| tolerated(key, condition, &current, &base, &tolerances))
    .collect();
    let score = |rule: &str, bound: f64, result: RuleOutcome| {
        gate_rule(rule, Some(0.75), Some(bound), result, None)
    };
    assert_eq!(
        rules,
        [
            with_tolerance(
                score(
                    "tests.mutation.score >= base",
                    0.76 - 0.02,
                    RuleOutcome::Pass
                ),
                0.02
            ),
            with_tolerance(
                score(
                    "tests.mutation.score > base",
                    0.76 - 0.02,
                    RuleOutcome::Pass
                ),
                0.02
            ),
            score("tests.mutation.score >= base - 0", 0.76, RuleOutcome::Fail),
            score("tests.mutation.score == base", 0.76, RuleOutcome::Fail),
            with_tolerance(
                gate_rule(
                    "static.crap.max <= base",
                    Some(20.0),
                    Some(20.0),
                    RuleOutcome::Pass,
                    None
                ),
                0.0
            ),
        ]
    );
}

#[test]
fn a_count_s_tolerance_raises_the_bound_of_an_upper_limit() {
    let current = measured();
    let reference = envelope(
        "pass",
        &[check("static.crap", "pass", &json!({"max": 19.0}))],
        &[],
    );
    let tolerances = Tolerances::from_iter([("static.crap.max".to_owned(), 1.0)]);
    let rule = tolerated(
        "static.crap.max",
        "< base",
        &current,
        &GateReference::Run(&reference),
        &tolerances,
    );
    assert_eq!(
        rule,
        with_tolerance(
            gate_rule(
                "static.crap.max < base",
                Some(20.0),
                Some(20.0),
                RuleOutcome::Fail,
                None
            ),
            1.0
        )
    );
}

/// A ratchet on `static.complexity.cyclomatic_max` from a base of 3 to `actual`, with δ = 2
/// and a watch threshold of 10, as `(bound, tolerance, watch, result)`.
fn ratcheted(actual: f64, condition: &str) -> (Option<f64>, Option<f64>, Option<f64>, RuleOutcome) {
    let key = "static.complexity.cyclomatic_max";
    let on = target("grade::letter", "src/grade.rs", 1);
    let (current, reference) = (complexity(actual, &on), complexity(3.0, &on));
    let tolerances = Tolerances::from_iter([(key.to_owned(), 2.0)]).watching(key, 10.0);
    let rule = tolerated(
        key,
        condition,
        &current,
        &GateReference::Run(&reference),
        &tolerances,
    );
    (rule.bound, rule.tolerance, rule.watch, rule.result)
}

#[test]
fn a_largest_value_ratchet_allows_its_tolerance_and_never_fails_within_the_watch_threshold() {
    use RuleOutcome::{Fail, Pass};
    assert_eq!(
        [
            ratcheted(5.0, "<= base"),
            ratcheted(8.0, "<= base"),
            ratcheted(10.0, "<= base"),
            ratcheted(11.0, "<= base"),
            ratcheted(9.0, "< base"),
        ],
        [
            (Some(5.0), Some(2.0), None, Pass),
            (Some(5.0), Some(2.0), Some(10.0), Pass),
            (Some(5.0), Some(2.0), Some(10.0), Pass),
            (Some(5.0), Some(2.0), None, Fail),
            (Some(5.0), Some(2.0), Some(10.0), Pass),
        ]
    );
    // A written offset replaces the tolerance and the watch threshold with it (ADR-205).
    assert_eq!(
        [ratcheted(4.0, "<= base + 0"), ratcheted(6.0, "<= base + 3")],
        [(Some(3.0), None, None, Fail), (Some(6.0), None, None, Pass)]
    );
}

#[test]
fn the_declared_watch_threshold_is_the_lowest_limit_among_the_scope_s_languages() {
    let dir = tempfile::tempdir().unwrap();
    let config = support::config_at(
        dir.path(),
        "[checks.\"static.complexity\"]\ncyclomatic_max = 8\n\
         [checks.\"static.complexity\".lang.go]\ncyclomatic_max = 6\n",
    );
    let mut manifest = support::manifest("static.complexity", &[], &[]);
    manifest.settings = ["cyclomatic_max", "cognitive_max"]
        .map(|name| SettingSpec {
            name: name.to_owned(),
            default: json!(15),
            summary: String::new(),
            configures: None,
        })
        .to_vec();
    manifest.metrics = ["cyclomatic_max", "cognitive_max", "length_max"]
        .map(|name| MetricSpec {
            name: name.to_owned(),
            summary: String::new(),
            per: MetricPer::Scope,
            protection: None,
            tolerance: Some(2.0),
            watch: (name != "length_max").then(|| name.to_owned()),
        })
        .to_vec();
    let mut checks = CheckRegistry::default();
    checks
        .register(Box::new(support::Passing(manifest)))
        .unwrap();
    let watch = |langs: &[&str], metric: &str| {
        let langs: Vec<LangId> = langs.iter().map(|lang| LangId::from(*lang)).collect();
        let key = GateRuleSpec::parse(&format!("static.complexity.{metric}"), "<= base")
            .unwrap()
            .key;
        Tolerances::declared(&checks, &config, &langs).watch(&key)
    };
    assert_eq!(
        [
            watch(&["rust"], "cyclomatic_max"),
            watch(&["rust", "go"], "cyclomatic_max"),
            watch(&[], "cyclomatic_max"),
            watch(&["rust"], "cognitive_max"),
            watch(&["rust"], "length_max"),
        ],
        [Some(8.0), Some(6.0), Some(8.0), Some(15.0), None]
    );
}
