// ABOUTME: The `CoverageMap` fact payload (design §8.3.2): line hits and optional branch hits per file, as
// ABOUTME: a coverage producer reports them (ADR-067), merged and sorted, with per-range queries (ADR-077).
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactKind};

/// Which lines (and, when the producer measures them, branches) the test suite ran.
///
/// Files are sorted by path, lines by number, and branches by line and index, each once; build
/// one with [`CoverageMap::new`] to get that shape from records in any order (ADR-077).
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

impl FileCoverage {
    /// The coverage of `file` from `(line, hits)` pairs and, when the producer measured
    /// branches, `(line, branch, hits)` triples.
    pub fn from_hits(
        file: &str,
        lines: &[(u32, u64)],
        branches: Option<&[(u32, u32, u64)]>,
    ) -> Self {
        Self {
            file: file.to_owned(),
            lines: lines
                .iter()
                .map(|&(line, hits)| LineHits { line, hits })
                .collect(),
            branches: branches.map(|branches| {
                branches
                    .iter()
                    .map(|&(line, branch, hits)| BranchHits { line, branch, hits })
                    .collect()
            }),
        }
    }
}

impl CoverageMap {
    /// The map of `records`: records of one file merge, a line's or a branch's hits add up
    /// across them, and a file has branches when any of its records measured them.
    pub fn new(records: Vec<FileCoverage>) -> Self {
        let mut files: BTreeMap<String, Merged> = BTreeMap::new();
        for record in records {
            let merged = files.entry(record.file).or_default();
            for hits in record.lines {
                let total = merged.lines.entry(hits.line).or_default();
                *total = total.saturating_add(hits.hits);
            }
            if let Some(branches) = record.branches {
                let measured = merged.branches.get_or_insert_with(BTreeMap::new);
                for hits in branches {
                    let total = measured.entry((hits.line, hits.branch)).or_default();
                    *total = total.saturating_add(hits.hits);
                }
            }
        }
        Self {
            files: files
                .into_iter()
                .map(|(file, merged)| FileCoverage {
                    file,
                    lines: merged
                        .lines
                        .into_iter()
                        .map(|(line, hits)| LineHits { line, hits })
                        .collect(),
                    branches: merged.branches.map(|branches| {
                        branches
                            .into_iter()
                            .map(|((line, branch), hits)| BranchHits { line, branch, hits })
                            .collect()
                    }),
                })
                .collect(),
        }
    }

    /// Whether the producer measured branches in any file; `tests.coverage` notes
    /// `branches: unavailable` when it measured none (design §8.3.2).
    pub fn branches_measured(&self) -> bool {
        self.files.iter().any(|file| file.branches.is_some())
    }

    /// The instrumented lines from `start` to `end` (inclusive) of `file`, in order.
    pub fn line_hits(&self, file: &str, start: u32, end: u32) -> Vec<&LineHits> {
        self.files
            .iter()
            .filter(|coverage| coverage.file == file)
            .flat_map(|coverage| &coverage.lines)
            .filter(|hits| (start..=end).contains(&hits.line))
            .collect()
    }

    /// The branches on the lines from `start` to `end` (inclusive) of `file`, in order; `None`
    /// when the producer did not measure branches in `file` (or did not report it).
    pub fn branch_hits(&self, file: &str, start: u32, end: u32) -> Option<Vec<&BranchHits>> {
        let branches = self
            .files
            .iter()
            .find(|coverage| coverage.file == file)?
            .branches
            .as_ref()?;
        Some(
            branches
                .iter()
                .filter(|hits| (start..=end).contains(&hits.line))
                .collect(),
        )
    }

    /// The share of instrumented lines from `start` to `end` (inclusive) of `file` that ran at
    /// least once; `None` when none of them is instrumented.
    pub fn line_coverage(&self, file: &str, start: u32, end: u32) -> Option<f64> {
        let lines = self.line_hits(file, start, end);
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

/// One file's hits while records are merged: by line, and by line and branch index.
#[derive(Default)]
struct Merged {
    lines: BTreeMap<u32, u64>,
    branches: Option<BTreeMap<(u32, u32), u64>>,
}
