// ABOUTME: What one check reports about a run: its status, score, scope metrics, delta, and why it was
// ABOUTME: skipped; below the envelope so checks and the scheduler need not depend on it.
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::FactKind;
use crate::grade::{CostClass, Tier};
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
