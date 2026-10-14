// ABOUTME: Measurements more than one check makes the same way, so their numbers agree: cyclomatic complexity
// ABOUTME: (ADR-024, ADR-042), a symbol as a ranking target, and combining `scope` metrics across languages (ADR-063).
use cupel_core::syntax::{Decision, SymbolDef};
use cupel_core::{CheckResult, Location, SymbolHash, Target};

/// 1 plus the cyclomatic weight of every decision.
pub fn cyclomatic(decisions: &[Decision]) -> u32 {
    1 + decisions.iter().map(|d| d.cyclomatic).sum::<u32>()
}

/// `symbol` as what a per-symbol measurement is about (ADR-062).
pub fn symbol_target(symbol: &SymbolDef, hash: Option<&SymbolHash>) -> Target {
    Target {
        display: symbol.display.clone(),
        symbol: hash.cloned(),
        location: Some(Location {
            file: symbol.file.clone(),
            line: symbol.span.line,
            col: Some(symbol.span.col),
            end_line: symbol.span.end_line,
            end_col: Some(symbol.span.end_col),
        }),
    }
}

/// A bound over the whole scope: the largest language's value, as `Check::combine_metric`'s
/// default.
pub fn largest(name: &str, results: &[&CheckResult]) -> Option<f64> {
    results
        .iter()
        .filter_map(|result| result.metrics.get(name))
        .copied()
        .reduce(f64::max)
}

/// A count over the whole scope: the languages' counts add up.
pub fn sum(name: &str, results: &[&CheckResult]) -> Option<f64> {
    results
        .iter()
        .filter_map(|result| result.metrics.get(name))
        .copied()
        .reduce(|total, value| total + value)
}

/// A ratio over the whole scope: each language's ratio weighted by what it is a ratio of, so
/// the result is the ratio over every language's targets together. Zero when nothing weighs.
pub fn weighted_mean(
    name: &str,
    results: &[&CheckResult],
    weight: impl Fn(&CheckResult) -> f64,
) -> Option<f64> {
    let mut total = 0.0;
    let mut weighted = 0.0;
    for result in results {
        let value = result.metrics.get(name)?;
        total += weight(result);
        weighted += value * weight(result);
    }
    (!results.is_empty()).then(|| if total == 0.0 { 0.0 } else { weighted / total })
}

/// The targets a language's score covers, as a weight.
#[allow(
    clippy::cast_precision_loss,
    reason = "target counts stay far below 2^52, where f64 is exact"
)]
pub fn scored(result: &CheckResult) -> f64 {
    result.score.as_ref().map_or(0.0, |score| score.n as f64)
}
