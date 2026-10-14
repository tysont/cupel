// ABOUTME: The `CoverageMap` fact payload (design §8.3.2): line hits and optional branch hits per file, as
// ABOUTME: a coverage producer reports them. Producers arrive in P3.2; checks may read it now (ADR-067).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactKind};

/// Which lines (and, when the producer measures them, branches) the test suite ran.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CoverageMap {
    pub files: Vec<FileCoverage>,
}

impl Fact for CoverageMap {
    const KIND: FactKind = FactKind::CoverageMap;
    const VERSION: u32 = 1;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FileCoverage {
    /// Relative to the repository root, `/`-separated.
    pub file: String,
    /// Every instrumented line; a line absent here is not executable code.
    pub lines: Vec<LineHits>,
    /// Branch hits; `None` when the producer does not measure branches.
    pub branches: Option<Vec<BranchHits>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LineHits {
    /// 1-based.
    pub line: u32,
    pub hits: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BranchHits {
    /// The 1-based line holding the branch.
    pub line: u32,
    /// The branch's index among the branches on its line.
    pub branch: u32,
    pub hits: u64,
}

impl CoverageMap {
    /// The share of instrumented lines from `start` to `end` (inclusive) of `file` that ran at
    /// least once; `None` when none of them is instrumented.
    pub fn line_coverage(&self, file: &str, start: u32, end: u32) -> Option<f64> {
        let lines: Vec<&LineHits> = self
            .files
            .iter()
            .filter(|coverage| coverage.file == file)
            .flat_map(|coverage| &coverage.lines)
            .filter(|hits| (start..=end).contains(&hits.line))
            .collect();
        if lines.is_empty() {
            return None;
        }
        let run = lines.iter().filter(|hits| hits.hits > 0).count();
        #[allow(
            clippy::cast_precision_loss,
            reason = "line counts stay far below 2^52, where f64 is exact"
        )]
        Some(run as f64 / lines.len() as f64)
    }
}
