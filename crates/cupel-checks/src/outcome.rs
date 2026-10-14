// ABOUTME: What every check returns the same way: its result for one language, the plan over the symbols
// ABOUTME: or files in scope, and scores combined across languages by sum or by maximum (ADR-041).
use std::collections::BTreeMap;

use cupel_core::{CheckResult, CheckStatus, Manifest, Plan, Scope, Score};

/// One language's result of the check `manifest` describes.
pub fn result(
    manifest: &Manifest,
    status: CheckStatus,
    score: Option<Score>,
    metrics: BTreeMap<String, f64>,
    findings: u64,
) -> CheckResult {
    CheckResult {
        id: manifest.id.clone(),
        status,
        tier: manifest.tier,
        cost: manifest.cost,
        score,
        metrics,
        delta: None,
        findings,
        duration_ms: 0,
        skipped: None,
        oracle: None,
    }
}

/// `fail` when the check reported something that fails it, else `pass`.
pub fn status(failed: bool) -> CheckStatus {
    if failed {
        CheckStatus::Fail
    } else {
        CheckStatus::Pass
    }
}

/// A plan over every symbol in `scope`, at `manifest`'s cost.
pub fn plan_symbols(manifest: &Manifest, scope: &Scope) -> Plan {
    let targets = scope.symbols.as_ref().map_or(0, |keys| keys.len() as u64);
    Plan::estimate(manifest.cost, targets)
}

/// A plan over every file in `scope`, at `manifest`'s cost.
pub fn plan_files(manifest: &Manifest, scope: &Scope) -> Plan {
    Plan::estimate(manifest.cost, scope.files.len() as u64)
}

/// Scores whose values add up across languages; their targets always do.
pub fn summed(scores: &[Score]) -> Option<Score> {
    combined(scores, |a, b| a + b)
}

/// Scores whose combined value is the largest language's; their targets add up.
pub fn maximum(scores: &[Score]) -> Option<Score> {
    combined(scores, f64::max)
}

fn combined(scores: &[Score], value: fn(f64, f64) -> f64) -> Option<Score> {
    let first = scores.first()?;
    Some(Score {
        value: scores.iter().map(|score| score.value).fold(0.0, value),
        unit: first.unit.clone(),
        n: scores.iter().map(|score| score.n).sum(),
    })
}
