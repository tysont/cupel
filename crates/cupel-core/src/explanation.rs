// ABOUTME: What `explain` adds to an envelope (design §6.5): the stored run a finding or check result
// ABOUTME: came from, a source excerpt, test hints, and per-symbol metrics; carried in `explanation`.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::hash::SymbolHash;
use crate::id::RunId;

/// Context for one finding or one check result, read from the fact store. The finding or check
/// result itself is in the envelope's `findings` or `checks`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Explanation {
    pub subject: ExplainSubject,
    /// The finding ID or check ID that was asked about.
    pub target: String,
    /// The stored run the explanation comes from: the latest run that produced the finding, or
    /// the latest run of the check.
    pub run: RunId,
    /// The source around the finding's location; null when it has no location or the file is
    /// gone.
    pub excerpt: Option<Excerpt>,
    /// Tests that cover the finding's site, from `action.hints.covering_tests`.
    pub covering_tests: Vec<String>,
    /// The test file nearest to the finding's site, from `action.hints.nearest_test_file`.
    pub nearest_test_file: Option<String>,
    /// The check's metrics in that run, per symbol; empty for a finding.
    pub metrics: Vec<Metric>,
}

/// What `explain` was asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ExplainSubject {
    Finding,
    Check,
}

/// Lines of one file around a location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Excerpt {
    /// Relative to the repository root.
    pub file: String,
    pub lines: Vec<ExcerptLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExcerptLine {
    /// 1-based.
    pub line: u32,
    pub text: String,
    /// Whether the line is inside the finding's location.
    pub focus: bool,
}

/// One measured value of a check, for a symbol or for the check as a whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Metric {
    /// Null for a value about the whole check, such as its score.
    pub symbol: Option<SymbolHash>,
    /// The metric's name, such as `cyclomatic` or `score`.
    pub key: String,
    pub value: f64,
}
