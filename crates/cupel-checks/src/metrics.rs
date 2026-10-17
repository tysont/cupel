// ABOUTME: Measurements more than one check makes the same way, so their numbers agree: per-symbol fact indexes,
// ABOUTME: test-case keys, cyclomatic complexity (ADR-024, ADR-042), ranking targets, and combining `scope` metrics (ADR-063).
use std::collections::HashMap;

use cupel_core::coverage::{CoverageMap, LineHits};
use cupel_core::syntax::{
    Decision, Decisions, NormNode, NormTree, SymbolDef, SymbolKey, TestCase, TestFile,
};
use cupel_core::{CheckResult, Location, SymbolHash, Target};

/// Each symbol's decisions, by symbol.
pub fn decisions_by_symbol(decisions: &Decisions) -> HashMap<&SymbolKey, &[Decision]> {
    decisions
        .symbols
        .iter()
        .map(|entry| (&entry.symbol, entry.decisions.as_slice()))
        .collect()
}

/// Each symbol's structural hash, by symbol (ADR-014).
pub fn hashes_by_symbol(trees: &NormTree) -> HashMap<&SymbolKey, &SymbolHash> {
    trees
        .symbols
        .iter()
        .map(|entry| (&entry.symbol, &entry.hash))
        .collect()
}

/// Each symbol's structural hash and normalized tree, by symbol.
pub fn trees_by_symbol<'a>(
    trees: impl IntoIterator<Item = &'a NormTree>,
) -> HashMap<&'a SymbolKey, (&'a SymbolHash, &'a NormNode)> {
    trees
        .into_iter()
        .flat_map(|fact| &fact.symbols)
        .map(|entry| (&entry.symbol, (&entry.hash, &entry.root)))
        .collect()
}

/// The key of the symbol a test case is: the case's node is the symbol's node (ADR-030, ADR-054),
/// so scope, `Symbols`, and `NormTree` join a case by it.
pub fn case_key(file: &TestFile, case: &TestCase) -> SymbolKey {
    SymbolKey {
        file: file.file.clone(),
        line: case.span.line,
        col: case.span.col,
    }
}

/// The instrumented lines that say whether tests ran `symbol`: those after its first line, or
/// its first line alone when nothing after it is instrumented. A declaration line runs when the
/// definition is evaluated (a Python `def` on import), not when the function is called
/// (ADR-079).
pub fn symbol_lines<'a>(coverage: &'a CoverageMap, symbol: &SymbolDef) -> Vec<&'a LineHits> {
    let (first, last) = (symbol.span.line, symbol.span.end_line);
    let body = coverage.line_hits(&symbol.file, first.saturating_add(1), last);
    if body.is_empty() {
        coverage.line_hits(&symbol.file, first, first)
    } else {
        body
    }
}

/// The share of `symbol`'s lines ([`symbol_lines`]) that ran; `None` when none is instrumented.
pub fn symbol_coverage(coverage: &CoverageMap, symbol: &SymbolDef) -> Option<f64> {
    let lines = symbol_lines(coverage, symbol);
    if lines.is_empty() {
        return None;
    }
    let ran = lines.iter().filter(|hits| hits.hits > 0).count();
    #[allow(
        clippy::cast_precision_loss,
        reason = "line counts stay far below 2^52, where f64 is exact"
    )]
    Some(ran as f64 / lines.len() as f64)
}

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
