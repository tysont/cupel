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
        notes: Vec::new(),
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

/// Ratios combined over every language's targets together: each language's value weighted by
/// the targets its score covers; zero when none does.
pub fn weighted(scores: &[Score]) -> Option<Score> {
    let first = scores.first()?;
    let targets: u64 = scores.iter().map(|score| score.n).sum();
    #[allow(
        clippy::cast_precision_loss,
        reason = "target counts stay far below 2^52, where f64 is exact"
    )]
    let value = if targets == 0 {
        0.0
    } else {
        scores
            .iter()
            .map(|score| score.value * score.n as f64)
            .sum::<f64>()
            / targets as f64
    };
    Some(Score {
        value,
        unit: first.unit.clone(),
        n: targets,
    })
}

fn combined(scores: &[Score], value: fn(f64, f64) -> f64) -> Option<Score> {
    let first = scores.first()?;
    Some(Score {
        value: scores.iter().map(|score| score.value).fold(0.0, value),
        unit: first.unit.clone(),
        n: scores.iter().map(|score| score.n).sum(),
    })
}

/// Counts as metrics, by name.
#[expect(
    clippy::cast_precision_loss,
    reason = "counts stay far below 2^52, where f64 is exact"
)]
pub fn counts(counts: &[(&str, usize)]) -> BTreeMap<String, f64> {
    counts
        .iter()
        .map(|(name, count)| ((*name).to_owned(), *count as f64))
        .collect()
}

/// A score of `value` things in `unit`, over `n` targets.
#[expect(
    clippy::cast_precision_loss,
    reason = "counts stay far below 2^52, where f64 is exact"
)]
pub fn counted(value: usize, unit: &str, n: usize) -> Score {
    Score {
        value: value as f64,
        unit: unit.to_owned(),
        n: n as u64,
    }
}

/// How many of `items` have the status `wanted`, as `status` reads it.
pub fn tally<T, S: PartialEq>(items: &[T], status: impl Fn(&T) -> S, wanted: &S) -> usize {
    items.iter().filter(|item| status(item) == *wanted).count()
}
