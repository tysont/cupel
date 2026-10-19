// ABOUTME: What one check reports about a run: its status, score, scope metrics, delta, and why it was
// ABOUTME: skipped; below the envelope so checks and the scheduler need not depend on it.
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::FactKind;
use crate::finding::Symbol;
use crate::grade::{CostClass, Tier};
use crate::hash::FindingId;
use crate::id::{CheckId, LangId, ToolId};

/// One check's outcome within a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckResult {
    pub id: CheckId,
    pub status: CheckStatus,
    pub tier: Tier,
    pub cost: CostClass,
    pub score: Option<Score>,
    /// The value of each `scope` metric the check measured, by metric name; the gate reads
    /// them as `<check-id>.<metric>` (ADR-060).
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
    pub delta: Option<Delta>,
    /// Number of findings this check produced.
    pub findings: u64,
    pub duration_ms: u64,
    pub skipped: Option<Skipped>,
    /// The oracle a differential or trace check compared against.
    pub oracle: Option<String>,
    /// What the result could not take into account, such as `branches: unavailable` when the
    /// coverage map measured no branches. Absent when empty: an optional property of an
    /// emitted type (with `Manifest.settings`, ADR-111), so results without notes are unchanged
    /// (ADR-079).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-cupel-optional" = true))]
    pub notes: Vec<String>,
}

/// A check's outcome (ADR-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    Pass,
    Fail,
    Partial,
    Error,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Score {
    pub value: f64,
    /// What `value` measures, such as `killed_ratio`.
    pub unit: String,
    /// How many targets the score covers.
    pub n: u64,
}

/// Change in score against a reference run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Delta {
    /// `baseline`, a git ref, or `run:<id>`.
    pub vs: String,
    pub value: f64,
    /// The reference run's symbols matched to this run's by similarity, where the check had
    /// findings or per-symbol metrics to compare across them (ADR-131). Absent when empty: an
    /// optional property of an emitted type, so deltas without matches are unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-cupel-optional" = true))]
    pub sets: Vec<SetDelta>,
}

/// A set of the reference run's symbols matched to a set of this run's (ADR-131): one to one for
/// a rename, a move, or an edit; one to many for a split; many to one for a merge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetDelta {
    /// The reference run's symbols.
    pub base: Vec<Symbol>,
    /// This run's symbols.
    pub head: Vec<Symbol>,
    /// The Jaccard similarity of the two sets' subtree fingerprints, to three decimals.
    pub similarity: f64,
    /// The check's findings carried across the match: neither resolved nor new.
    pub findings: SetFindings,
    /// Each per-symbol metric measured on both sides, compared across the sets, by name.
    pub metrics: BTreeMap<String, SetMetric>,
}

/// Findings of one check carried across a match: each of the reference run's findings on the
/// base set has a finding of the same kind on the head set in this run, and the reverse.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetFindings {
    /// The reference run's findings on the base set.
    pub base: Vec<FindingId>,
    /// This run's findings on the head set.
    pub head: Vec<FindingId>,
}

/// One per-symbol metric over a matched set: the sum of a count, or the mean of a score weighted
/// by each symbol's tree size, over the members measured.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetMetric {
    /// Over the base set, in the reference run.
    pub base: f64,
    /// Over the head set, in this run.
    pub head: f64,
}

/// Why a check did not run (design §8.3.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Skipped {
    pub reason: SkipReason,
    /// The missing fact, for `unsupported`.
    pub fact: Option<FactKind>,
    pub lang: Option<LangId>,
    /// The missing tool, for `unavailable`.
    pub tool: Option<ToolId>,
    /// A command that explains the reason.
    pub see: Option<String>,
    /// What the skipped work was estimated to cost, for `budget`: the check's own cost plus the
    /// producer runs it would have brought in (ADR-086 §10).
    pub est_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SkipReason {
    Unsupported,
    Unavailable,
    Unconfigured,
    Budget,
    Timeout,
    // The check compares a range's base and head, and the scope has no range (ADR-056).
    NeedsRange,
}
