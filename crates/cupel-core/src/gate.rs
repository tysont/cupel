// ABOUTME: Gate evaluation (design §6.7, ADR-060, ADR-130): picks `[gate.changed]` or `[gate.tree]` by scope mode,
// ABOUTME: evaluates each rule against a graded envelope's metrics, findings, reference run, and tolerances, and applies the result.
use std::collections::BTreeMap;

use crate::check::CheckRegistry;
use crate::config::{Comparison, GateCondition, GateConfig, GateKey, GateRuleSpec, GateSection};
use crate::envelope::{Envelope, GateOutcome, GateResult, GateRule, RuleOutcome};
use crate::finding::Severity;
use crate::rank::Target;
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

/// Each gate key's tolerance, as `<check>.<metric>` (ADR-182, ADR-195): how far a rule against
/// `base` that writes no offset lets the metric move past its reference value. A key not held
/// has tolerance 0.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tolerances(BTreeMap<String, f64>);

impl Tolerances {
    /// The tolerances the registered checks' manifests declare on their metrics.
    pub fn declared(checks: &CheckRegistry) -> Self {
        checks
            .iter()
            .flat_map(|check| {
                let manifest = check.manifest();
                manifest.metrics.iter().filter_map(|metric| {
                    let tolerance = metric.tolerance?;
                    Some((format!("{}.{}", manifest.id, metric.name), tolerance))
                })
            })
            .collect()
    }

    /// The tolerance of `key`'s metric; 0 when none is declared.
    pub fn of(&self, key: &GateKey) -> f64 {
        self.0.get(&key.to_string()).copied().unwrap_or(0.0)
    }
}

impl FromIterator<(String, f64)> for Tolerances {
    fn from_iter<I: IntoIterator<Item = (String, f64)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
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
/// against `base` compares with `reference`'s value of the same metric, loosened by the metric's
/// tolerance in `tolerances` unless the rule writes an offset.
pub fn evaluate_gate(
    evaluated: &str,
    rules: &[GateRuleSpec],
    envelope: &Envelope,
    reference: &GateReference,
    tolerances: &Tolerances,
) -> GateResult {
    let rules: Vec<GateRule> = rules
        .iter()
        .map(|rule| {
            let evaluation = evaluate_rule(rule, envelope, reference, tolerances);
            let (actual_target, bound_target) = targets(rule, envelope, reference, &evaluation);
            GateRule {
                rule: rule.to_string(),
                actual: evaluation.actual,
                actual_target,
                bound: evaluation.bound,
                bound_target,
                tolerance: evaluation.tolerance,
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
    /// The tolerance that loosened `bound`, when one did.
    tolerance: Option<f64>,
    result: RuleOutcome,
    reason: Option<String>,
}

impl Evaluation {
    fn compared(actual: f64, bound: f64, passed: bool) -> Self {
        Self {
            actual: Some(actual),
            bound: Some(bound),
            tolerance: None,
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
            tolerance: None,
            result: RuleOutcome::Skipped,
            reason: Some(reason),
        }
    }
}

fn evaluate_rule(
    rule: &GateRuleSpec,
    envelope: &Envelope,
    reference: &GateReference,
    tolerances: &Tolerances,
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
            tolerance: None,
            result: RuleOutcome::Pass,
            reason: None,
        };
    }
    let Some(entry) = completed(envelope, rule) else {
        return Evaluation::skipped(None, incomplete(envelope, rule));
    };
    match rule.condition {
        GateCondition::Compare { op, threshold } => {
            let bound = if threshold.percent {
                threshold.value / 100.0
            } else {
                threshold.value
            };
            compared_metric(rule, entry, op, Ok(bound))
        }
        GateCondition::Relative { op, offset } => {
            let (offset, tolerance) = match offset {
                Some(offset) => (offset, None),
                None => loosened(op, tolerances.of(&rule.key)),
            };
            let bound = reference_value(rule, reference).map(|base| base + offset);
            let mut evaluation = compared_metric(rule, entry, op, bound);
            if evaluation.bound.is_some() {
                evaluation.tolerance = tolerance;
            }
            evaluation
        }
        GateCondition::None | GateCondition::NoBreaking => presence(rule, envelope, entry),
    }
}

/// The offset from `base` that `tolerance` gives a rule under `op`, with the tolerance applied:
/// below the reference for a lower limit, above it for an upper one, and none for `==`.
fn loosened(op: Comparison, tolerance: f64) -> (f64, Option<f64>) {
    match op {
        Comparison::Ge | Comparison::Gt => (-tolerance, Some(tolerance)),
        Comparison::Le | Comparison::Lt => (tolerance, Some(tolerance)),
        Comparison::Eq => (0.0, None),
    }
}

/// A comparison of the rule's metric in `entry` with `bound`, or why there is no bound.
fn compared_metric(
    rule: &GateRuleSpec,
    entry: &CheckResult,
    op: Comparison,
    bound: Result<f64, String>,
) -> Evaluation {
    let Some(actual) = metric(entry, rule) else {
        return Evaluation::skipped(None, unreported(rule));
    };
    match bound {
        Ok(bound) => Evaluation::compared(actual, bound, compare(op, actual, bound)),
        Err(why) => Evaluation::skipped(Some(actual), format!("{NO_REFERENCE}: {why}")),
    }
}

/// `none` or `no-breaking`: the count of the check's unsuppressed findings above `info` (only
/// the breaking ones for `no-breaking`) must be 0.
fn presence(rule: &GateRuleSpec, envelope: &Envelope, entry: &CheckResult) -> Evaluation {
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

/// What the rule's `actual` and `bound` were measured on (the checks' `metric_targets`), when
/// each was measured: the current run's, and the reference run's for a rule against `base`.
fn targets(
    rule: &GateRuleSpec,
    envelope: &Envelope,
    reference: &GateReference,
    evaluation: &Evaluation,
) -> (Option<Target>, Option<Target>) {
    let target = |run: &Envelope| {
        let metric = rule.key.metric.as_ref()?;
        completed(run, rule)?.metric_targets.get(metric).cloned()
    };
    let actual = evaluation.actual.and_then(|_| target(envelope));
    let bound = match (reference, &rule.condition, evaluation.bound) {
        (GateReference::Run(run), GateCondition::Relative { .. }, Some(_)) => target(run),
        _ => None,
    };
    (actual, bound)
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
/// Why a rule's check has no completed result: the run did not select it, every result of it
/// was skipped (with the skips' reasons), or it errored or was cut short.
fn incomplete(envelope: &Envelope, rule: &GateRuleSpec) -> String {
    let check = &rule.key.check;
    let entries: Vec<&CheckResult> = envelope
        .checks
        .iter()
        .filter(|entry| entry.id == *check)
        .collect();
    if entries.is_empty() {
        return format!("`{check}` was not selected for this run");
    }
    let mut reasons: Vec<String> = Vec::new();
    for entry in &entries {
        let Some(skipped) = entry.skipped.as_ref() else {
            return format!("`{check}` did not run to completion");
        };
        let reason = serde_json::to_value(skipped.reason)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default();
        if !reasons.contains(&reason) {
            reasons.push(reason);
        }
    }
    format!("`{check}` was skipped ({})", reasons.join(", "))
}

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
