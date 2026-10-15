// ABOUTME: Parses Go's coverprofile (`go test -coverprofile`): statement blocks with counts, by import path,
// ABOUTME: into a `CoverageMap` whose lines take their blocks' largest count and which measures no branches.
use std::collections::BTreeMap;

use cupel_core::coverage::{CoverageMap, FileCoverage, LineHits};

use super::error::ReportError;

const FORMAT: &str = "coverprofile";

/// The coverage in the coverprofile `text`, with each file named by its import path
/// (`example.com/grade/grade.go`), as the profile names it. Every line a block spans is
/// instrumented and has the largest count of the blocks spanning it; the format has no
/// branches, so every file's `branches` is `None`.
///
/// # Errors
///
/// When the `mode:` line is missing or a block line is malformed.
pub fn parse_coverprofile(text: &str) -> Result<CoverageMap, ReportError> {
    let mut lines = text.lines();
    if !lines.next().is_some_and(|first| first.starts_with("mode:")) {
        return Err(ReportError::new(FORMAT, "no `mode:` line".to_owned()));
    }
    let mut files: BTreeMap<String, BTreeMap<u32, u64>> = BTreeMap::new();
    for (index, line) in lines.enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let block = block(line).ok_or_else(|| {
            ReportError::new(FORMAT, format!("line {}: malformed block", index + 2))
        })?;
        let hits = files.entry(block.file.to_owned()).or_default();
        for line in block.start..=block.end {
            let most = hits.entry(line).or_default();
            *most = (*most).max(block.count);
        }
    }
    Ok(CoverageMap::new(
        files
            .into_iter()
            .map(|(file, hits)| FileCoverage {
                file,
                lines: hits
                    .into_iter()
                    .map(|(line, hits)| LineHits { line, hits })
                    .collect(),
                branches: None,
            })
            .collect(),
    ))
}

/// One block: `file:startLine.startCol,endLine.endCol statements count`.
struct Block<'a> {
    file: &'a str,
    start: u32,
    end: u32,
    count: u64,
}

fn block(line: &str) -> Option<Block<'_>> {
    let (file, rest) = line.rsplit_once(':')?;
    let mut words = rest.split_whitespace();
    let (from, to) = words.next()?.split_once(',')?;
    let _statements: u64 = words.next()?.parse().ok()?;
    let count = words.next()?.parse().ok()?;
    let line_of = |position: &str| position.split_once('.')?.0.parse().ok();
    Some(Block {
        file,
        start: line_of(from)?,
        end: line_of(to)?,
        count,
    })
}
