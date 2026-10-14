// ABOUTME: Parses LCOV tracefiles (`SF`/`DA`/`BRDA` records), which cargo-llvm-cov, coverage.py, jest, vitest,
// ABOUTME: and c8 write, into a `CoverageMap` with paths as written; a record with `BRDA` measured branches.
use std::collections::BTreeMap;

use cupel_core::coverage::{BranchHits, CoverageMap, FileCoverage, LineHits};

use super::error::ReportError;

const FORMAT: &str = "LCOV";

/// The coverage in the LCOV tracefile `text`, with each file's path as its `SF` line spells it.
/// A record has branches exactly when it holds a `BRDA` line (llvm-cov writes `BRF:0` when it
/// measured none); a branch's index is its place among the `BRDA` lines of its line, and a
/// `-` (never evaluated) counts as 0 hits. Records of one file merge, adding hits.
///
/// # Errors
///
/// When a `DA` or `BRDA` line is malformed or comes before any `SF`.
pub fn parse_lcov(text: &str) -> Result<CoverageMap, ReportError> {
    let mut reader = Reader::default();
    for (index, line) in text.lines().enumerate() {
        reader.read(index + 1, line.trim())?;
    }
    reader.close();
    Ok(CoverageMap::new(reader.records))
}

/// The records read so far and the one being read.
#[derive(Default)]
struct Reader {
    records: Vec<FileCoverage>,
    current: Option<Record>,
}

impl Reader {
    /// Reads line `number`, `line`; lines other than `SF`, `DA`, `BRDA`, and `end_of_record`
    /// carry nothing the map holds.
    fn read(&mut self, number: usize, line: &str) -> Result<(), ReportError> {
        if let Some(path) = line.strip_prefix("SF:") {
            self.close();
            self.current = Some(Record::new(path));
        } else if line == "end_of_record" {
            self.close();
        } else if let Some(fields) = line.strip_prefix("DA:") {
            let mut parts = fields.split(',');
            let line = field(parts.next(), number)?;
            let hits = field(parts.next(), number)?;
            let total = self.record(number)?.lines.entry(line).or_default();
            *total = total.saturating_add(hits);
        } else if let Some(fields) = line.strip_prefix("BRDA:") {
            let (line, hits) = branch(fields, number)?;
            self.record(number)?
                .branches
                .entry(line)
                .or_default()
                .push(hits);
        }
        Ok(())
    }

    fn close(&mut self) {
        self.records.extend(self.current.take().map(Record::finish));
    }

    fn record(&mut self, number: usize) -> Result<&mut Record, ReportError> {
        self.current.as_mut().ok_or_else(|| {
            ReportError::new(FORMAT, format!("line {number}: data before any SF line"))
        })
    }
}

/// `(line, hits)` of a `BRDA:<line>,<block>,<branch>,<taken>` line's fields; `-` is 0 hits.
fn branch(fields: &str, number: usize) -> Result<(u32, u64), ReportError> {
    let parts: Vec<&str> = fields.split(',').collect();
    let [line, _, _, taken] = parts[..] else {
        return Err(ReportError::new(
            FORMAT,
            format!("line {number}: malformed BRDA"),
        ));
    };
    let hits = if taken == "-" {
        0
    } else {
        field(Some(taken), number)?
    };
    Ok((field(Some(line), number)?, hits))
}

/// One `SF` record while it is read.
struct Record {
    file: String,
    lines: BTreeMap<u32, u64>,
    /// Hits of each branch, by line, in `BRDA` order.
    branches: BTreeMap<u32, Vec<u64>>,
}

impl Record {
    fn new(path: &str) -> Self {
        Self {
            file: path.to_owned(),
            lines: BTreeMap::new(),
            branches: BTreeMap::new(),
        }
    }

    fn finish(self) -> FileCoverage {
        let measured = !self.branches.is_empty();
        FileCoverage {
            file: self.file,
            lines: self
                .lines
                .into_iter()
                .map(|(line, hits)| LineHits { line, hits })
                .collect(),
            branches: measured.then(|| {
                self.branches
                    .into_iter()
                    .flat_map(|(line, hits)| {
                        hits.into_iter()
                            .zip(0u32..)
                            .map(move |(hits, branch)| BranchHits { line, branch, hits })
                    })
                    .collect()
            }),
        }
    }
}

fn field<T: std::str::FromStr>(text: Option<&str>, number: usize) -> Result<T, ReportError> {
    text.and_then(|text| text.trim().parse().ok())
        .ok_or_else(|| ReportError::new(FORMAT, format!("line {number}: malformed number")))
}
