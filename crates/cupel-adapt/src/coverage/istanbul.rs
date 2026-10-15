// ABOUTME: Parses Istanbul's JSON coverage (`coverage-final.json`, from jest and vitest) into a `CoverageMap`:
// ABOUTME: statement counts by start line (largest per line) and branch counts by the branch's line.
use std::collections::BTreeMap;

use cupel_core::coverage::{BranchHits, CoverageMap, FileCoverage, LineHits};
use serde::Deserialize;
use serde_json::Value;

use super::error::ReportError;

const FORMAT: &str = "Istanbul JSON";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct File {
    path: String,
    #[serde(default)]
    statement_map: BTreeMap<String, Location>,
    #[serde(default)]
    s: BTreeMap<String, u64>,
    #[serde(default)]
    branch_map: BTreeMap<String, Branch>,
    #[serde(default)]
    b: BTreeMap<String, Vec<u64>>,
}

#[derive(Deserialize)]
struct Location {
    start: Position,
}

#[derive(Deserialize)]
struct Position {
    line: Option<u32>,
}

#[derive(Deserialize)]
struct Branch {
    line: Option<u32>,
    loc: Option<Location>,
}

/// The coverage in the Istanbul JSON `text`, with each file's path as its `path` field spells it.
/// A line's hits are the largest count of the statements starting on it, as Istanbul's own LCOV
/// does; each branch's line is its `line` (or its location's start), and its index counts the
/// branches of that line in branch-ID order. A file has branches when its `branchMap` is not
/// empty.
///
/// # Errors
///
/// When `text` is not an object of Istanbul file coverage.
pub fn parse_istanbul(text: &str) -> Result<CoverageMap, ReportError> {
    let report: BTreeMap<String, Value> =
        serde_json::from_str(text).map_err(|error| ReportError::new(FORMAT, error.to_string()))?;
    let mut files = Vec::new();
    for value in report.into_values() {
        let file: File = serde_json::from_value(value)
            .map_err(|error| ReportError::new(FORMAT, error.to_string()))?;
        files.push(coverage(file));
    }
    Ok(CoverageMap::new(files))
}

fn coverage(file: File) -> FileCoverage {
    let mut lines: BTreeMap<u32, u64> = BTreeMap::new();
    for (id, location) in &file.statement_map {
        let (Some(line), Some(&hits)) = (location.start.line, file.s.get(id)) else {
            continue;
        };
        let most = lines.entry(line).or_default();
        *most = (*most).max(hits);
    }
    let mut ids: Vec<(&String, &Branch)> = file.branch_map.iter().collect();
    ids.sort_by_key(|(id, _)| id.parse::<u64>().unwrap_or(u64::MAX));
    let mut on_line: BTreeMap<u32, Vec<u64>> = BTreeMap::new();
    for (id, branch) in ids {
        let line = branch
            .line
            .or_else(|| branch.loc.as_ref().and_then(|loc| loc.start.line));
        let (Some(line), Some(counts)) = (line, file.b.get(id)) else {
            continue;
        };
        on_line.entry(line).or_default().extend(counts);
    }
    FileCoverage {
        file: file.path,
        lines: lines
            .into_iter()
            .map(|(line, hits)| LineHits { line, hits })
            .collect(),
        branches: (!on_line.is_empty()).then(|| {
            on_line
                .into_iter()
                .flat_map(|(line, counts)| {
                    counts
                        .into_iter()
                        .zip(0u32..)
                        .map(move |(hits, branch)| BranchHits { line, branch, hits })
                })
                .collect()
        }),
    }
}
