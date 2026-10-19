// ABOUTME: Gate evaluation (design §6.7, ADR-060, ADR-130): picks `[gate.changed]` or `[gate.tree]` by scope mode,
// ABOUTME: evaluates each rule against a graded envelope's metrics, findings, and reference run, and applies the result.
use crate::config::{Comparison, GateCondition, GateConfig, GateRuleSpec, GateSection};
use crate::envelope::{Envelope, GateOutcome, GateResult, GateRule, RuleOutcome};
use crate::finding::Severity;
use crate::result::{CheckResult, CheckStatus};
use crate::scope::ScopeMode;
use crate::status::{ExitCode, Status};

/// `summary.gate.evaluated` for `[gate.changed]`.
pub const GATE_CHANGED: &str = "gate.changed";
/// `summary.gate.evaluated` for `[gate.tree]`.
pub const GATE_TREE: &str = "gate.tree";

/// The prefix of a `skipped` rule's reason when the rule has no reference value (ADR-130).
pub const NO_REFERENCE: &str = "no reference";

/// What `base` stands for in a rule against the reference value (ADR-130): the run the current
/// one is compared with, or why there is none.
#[derive(Debug, Clone, PartialEq)]
pub enum GateReference<'a> {
    Run(&'a Envelope),
    /// Why no reference run exists, such as `no baseline is set`.
    Missing(String),
}

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

/// Whether any of `rules` compares with the reference value, so grading needs a reference run.
pub fn needs_reference(rules: &[GateRuleSpec]) -> bool {
    rules
        .iter()
        .any(|rule| matches!(rule.condition, GateCondition::Relative { .. }))
}

/// Evaluates `rules` (the table named `evaluated`) against a graded envelope: its suppressions
/// and deltas are already applied, so suppressed findings are `info` and do not count. A rule
/// against `base` compares with `reference`'s value of the same metric.
pub fn evaluate_gate(
    evaluated: &str,
    rules: &[GateRuleSpec],
    envelope: &Envelope,
    reference: &GateReference,
) -> GateResult {
    let rules: Vec<GateRule> = rules
        .iter()
        .map(|rule| {
            let evaluation = evaluate_rule(rule, envelope, reference);
            GateRule {
                rule: rule.to_string(),
                actual: evaluation.actual,
                bound: evaluation.bound,
                result: evaluation.result,
                reason: evaluation.reason,
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

/// One rule's measured value, the value it was compared with, and its outcome.
struct Evaluation {
    actual: Option<f64>,
    bound: Option<f64>,
    result: RuleOutcome,
    reason: Option<String>,
}

impl Evaluation {
    fn compared(actual: f64, bound: f64, passed: bool) -> Self {
        Self {
            actual: Some(actual),
            bound: Some(bound),
            result: if passed {
                RuleOutcome::Pass
            } else {
                RuleOutcome::Fail
            },
            reason: None,
        }
    }

    fn skipped(actual: Option<f64>, reason: String) -> Self {
        Self {
            actual,
            bound: None,
            result: RuleOutcome::Skipped,
            reason: Some(reason),
        }
    }
}

fn evaluate_rule(
    rule: &GateRuleSpec,
    envelope: &Envelope,
    reference: &GateReference,
) -> Evaluation {
    // Nothing in scope (an unchanged `--changed`) passes every rule vacuously (ADR-060 §6).
    if envelope
        .run
        .scope
        .as_ref()
        .is_some_and(|scope| scope.files == 0)
    {
        return Evaluation {
            actual: None,
            bound: None,
            result: RuleOutcome::Pass,
            reason: None,
        };
    }
    let Some(entry) = completed(envelope, rule) else {
        return Evaluation::skipped(
            None,
            format!("`{}` did not run to completion", rule.key.check),
        );
    };
    match rule.condition {
        GateCondition::Compare { op, threshold } => {
            let Some(actual) = metric(entry, rule) else {
                return Evaluation::skipped(None, unreported(rule));
            };
            let bound = if threshold.percent {
                threshold.value / 100.0
            } else {
                threshold.value
            };
            Evaluation::compared(actual, bound, compare(op, actual, bound))
        }
        GateCondition::Relative { op, offset } => {
            let Some(actual) = metric(entry, rule) else {
                return Evaluation::skipped(None, unreported(rule));
            };
            match reference_value(rule, reference) {
                Ok(base) => {
                    let bound = base + offset;
                    Evaluation::compared(actual, bound, compare(op, actual, bound))
                }
                Err(why) => Evaluation::skipped(Some(actual), format!("{NO_REFERENCE}: {why}")),
            }
        }
        GateCondition::None | GateCondition::NoBreaking => {
            let breaking_only = rule.condition == GateCondition::NoBreaking;
            let present = envelope
                .findings
                .iter()
                .filter(|finding| finding.check == entry.id && finding.severity != Severity::Info)
                .filter(|finding| !breaking_only || is_breaking(&finding.kind))
                .count();
            #[allow(
                clippy::cast_precision_loss,
                reason = "finding counts stay far below 2^52, where f64 is exact"
            )]
            let actual = present as f64;
            Evaluation::compared(actual, 0.0, present == 0)
        }
    }
}

/// The rule's metric as `entry` reports it.
fn metric(entry: &CheckResult, rule: &GateRuleSpec) -> Option<f64> {
    rule.key
        .metric
        .as_ref()
        .and_then(|metric| entry.metrics.get(metric))
        .copied()
}

/// Why a rule whose check completed was not evaluated: the check did not report its metric.
fn unreported(rule: &GateRuleSpec) -> String {
    format!(
        "`{}` did not report `{}`",
        rule.key.check,
        rule.key.metric.as_deref().unwrap_or_default()
    )
}

/// The rule's metric in the reference run, or why there is none.
fn reference_value(rule: &GateRuleSpec, reference: &GateReference) -> Result<f64, String> {
    let run = match reference {
        GateReference::Run(run) => run,
        GateReference::Missing(why) => return Err(why.clone()),
    };
    completed(run, rule)
        .and_then(|entry| metric(entry, rule))
        .ok_or_else(|| format!("the reference run did not report `{}`", rule.key))
}

/// Whether a finding of `kind` reports a breaking change: the kind's last segment is
/// `breaking`, such as `api.breaking` (ADR-101 §6).
pub fn is_breaking(kind: &str) -> bool {
    kind.rsplit('.').next() == Some("breaking") && kind.contains('.')
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
