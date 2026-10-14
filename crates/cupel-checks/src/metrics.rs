// ABOUTME: Per-symbol measurements more than one check computes the same way, so their numbers agree:
// ABOUTME: cyclomatic complexity from the weights the Decisions producer attached (ADR-024, ADR-042).
use cupel_core::syntax::Decision;

/// 1 plus the cyclomatic weight of every decision.
pub fn cyclomatic(decisions: &[Decision]) -> u32 {
    1 + decisions.iter().map(|d| d.cyclomatic).sum::<u32>()
}
