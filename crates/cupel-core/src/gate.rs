// ABOUTME: Gate evaluation (design §6.7, ADR-060): picks `[gate.changed]` or `[gate.tree]` by scope mode,
// ABOUTME: evaluates each rule against a graded envelope's check metrics and findings, and applies the result.
use crate::config::{Comparison, GateCondition, GateConfig, GateRuleSpec, GateSection};
use crate::envelope::{
    CheckResult, CheckStatus, Envelope, ExitCode, GateOutcome, GateResult, GateRule, RuleOutcome,
    ScopeMode, Status,
};
use crate::finding::Severity;

/// `summary.gate.evaluated` for `[gate.changed]`.
pub const GATE_CHANGED: &str = "gate.changed";
/// `summary.gate.evaluated` for `[gate.tree]`.
pub const GATE_TREE: &str = "gate.tree";

/// The gate table a scope is graded by, as `(name, rules)`: `[gate.changed]` for change-based
/// scopes, `[gate.tree]` otherwise. The rules are `None` when the configuration lacks the table.
pub fn gate_for(config: &GateConfig, mode: ScopeMode) -> (&'static str, Option<&GateSection>) {
    match mode {
        ScopeMode::Changed | ScopeMode::Range | ScopeMode::SinceBaseline => {
            (GATE_CHANGED, config.changed.as_ref())
        }
        ScopeMode::Tree | ScopeMode::Paths | ScopeMode::Symbols => {
            (GATE_TREE, config.tree.as_ref())
        }
    }
}

/// Evaluates `rules` (the table named `evaluated`) against a graded envelope: its suppressions
/// and deltas are already applied, so suppressed findings are `info` and do not count.
pub fn evaluate_gate(evaluated: &str, rules: &[GateRuleSpec], envelope: &Envelope) -> GateResult {
    let rules: Vec<GateRule> = rules
        .iter()
        .map(|rule| {
            let (actual, result) = evaluate_rule(rule, envelope);
            GateRule {
                rule: rule.to_string(),
                actual,
                result,
            }
        })
        .collect();
    let result = if rules.iter().all(|rule| rule.result == RuleOutcome::Pass) {
        GateOutcome::Pass
    } else {
        GateOutcome::Fail
    };
    GateResult {
        result,
        evaluated: evaluated.to_owned(),
        rules,
    }
}

/// Records `gate` in `summary.gate`; a failed gate turns a `pass` envelope into `fail`, exit 1.
/// A `partial` or `error` envelope keeps its status (ADR-060 §7).
pub fn apply_gate(envelope: &mut Envelope, gate: GateResult) {
    if gate.result == GateOutcome::Fail && envelope.status == Status::Pass {
        envelope.status = Status::Fail;
        envelope.exit_code = ExitCode::Fail;
    }
    envelope.summary.gate = Some(gate);
}

fn evaluate_rule(rule: &GateRuleSpec, envelope: &Envelope) -> (Option<f64>, RuleOutcome) {
    // Nothing in scope (an unchanged `--changed`) passes every rule vacuously (ADR-060 §6).
    if envelope
        .run
        .scope
        .as_ref()
        .is_some_and(|scope| scope.files == 0)
    {
        return (None, RuleOutcome::Pass);
    }
    let Some(entry) = completed(envelope, rule) else {
        return (None, RuleOutcome::Skipped);
    };
    match rule.condition {
        GateCondition::Compare { op, threshold } => {
            let Some(actual) = rule
                .key
                .metric
                .as_ref()
                .and_then(|metric| entry.metrics.get(metric))
                .copied()
            else {
                return (None, RuleOutcome::Skipped);
            };
            let bound = if threshold.percent {
                threshold.value / 100.0
            } else {
                threshold.value
            };
            (Some(actual), outcome(compare(op, actual, bound)))
        }
        GateCondition::None => {
            let present = envelope
                .findings
                .iter()
                .filter(|finding| finding.check == entry.id && finding.severity != Severity::Info)
                .count();
            #[allow(
                clippy::cast_precision_loss,
                reason = "finding counts stay far below 2^52, where f64 is exact"
            )]
            let actual = present as f64;
            (Some(actual), outcome(present == 0))
        }
        GateCondition::NoBreaking => (None, RuleOutcome::Unsupported),
    }
}

/// The rule's check entry when the check ran to completion for some language: not skipped
/// everywhere, not failed, not cut short.
fn completed<'a>(envelope: &'a Envelope, rule: &GateRuleSpec) -> Option<&'a CheckResult> {
    envelope.checks.iter().find(|entry| {
        entry.id == rule.key.check && matches!(entry.status, CheckStatus::Pass | CheckStatus::Fail)
    })
}

fn compare(op: Comparison, actual: f64, bound: f64) -> bool {
    match op {
        Comparison::Ge => actual >= bound,
        Comparison::Gt => actual > bound,
        Comparison::Le => actual <= bound,
        Comparison::Lt => actual < bound,
        #[allow(
            clippy::float_cmp,
            reason = "`==` in a gate rule means exactly equal (ADR-060 §6)"
        )]
        Comparison::Eq => actual == bound,
    }
}

fn outcome(passed: bool) -> RuleOutcome {
    if passed {
        RuleOutcome::Pass
    } else {
        RuleOutcome::Fail
    }
}
