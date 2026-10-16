// ABOUTME: The `PerTestMap` fact payload (ADR-077): the lines each test executes, named by its runner's test
// ABOUTME: IDs, and the inverted index `file → line → tests` that impact and mutant selection query.
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactKind};
use crate::id::ProducerId;

/// Which lines each test executes when it runs alone (ADR-077).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PerTestMap {
    /// The `TestResults` producer whose test IDs `tests` uses, such as `tool.cargo-nextest`; it is
    /// the runner a selection of these tests is handed to.
    pub runner: ProducerId,
    /// Every measured test, sorted by ID; a test that executed nothing has no `covered` entries.
    pub tests: Vec<TestCoverage>,
    /// How many of `tests` the production that made this map measured (ADR-083); 0 when every
    /// test came from the producer's cache.
    pub measured: u64,
    /// How many of `tests` came from the producer's cache without being measured again.
    pub reused: u64,
}

impl Fact for PerTestMap {
    const KIND: FactKind = FactKind::PerTestMap;
    const VERSION: u32 = 2;
}

/// The lines one test executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestCoverage {
    /// The runner's ID of the test (ADR-075).
    pub id: String,
    /// The test's own file, relative to the repository root, when the coverage tool names it.
    pub file: Option<String>,
    /// Sorted by file, one entry per file.
    pub covered: Vec<CoveredLines>,
}

/// The lines of one file a test executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CoveredLines {
    /// Relative to the repository root, `/`-separated.
    pub file: String,
    /// 1-based, ascending, unique.
    pub lines: Vec<u32>,
}

impl PerTestMap {
    /// The map of `runner`'s `tests`, sorted by ID, every one of them measured.
    pub fn new(runner: ProducerId, tests: Vec<TestCoverage>) -> Self {
        let measured = tests.len() as u64;
        Self::refreshed(runner, tests, measured)
    }

    /// The map of `runner`'s `tests`, sorted by ID, of which `measured` were measured and the
    /// rest came from a cache.
    pub fn refreshed(runner: ProducerId, mut tests: Vec<TestCoverage>, measured: u64) -> Self {
        tests.sort_by(|a, b| a.id.cmp(&b.id));
        let reused = (tests.len() as u64).saturating_sub(measured);
        Self {
            runner,
            tests,
            measured,
            reused,
        }
    }

    /// Whether every test came from the producer's cache.
    pub fn cached(&self) -> bool {
        self.measured == 0
    }

    /// The inverted index: for each file and line, the tests that execute it.
    pub fn index(&self) -> LineIndex<'_> {
        let mut by_file: BTreeMap<&str, BTreeMap<u32, Vec<usize>>> = BTreeMap::new();
        for (position, test) in self.tests.iter().enumerate() {
            for covered in &test.covered {
                let lines = by_file.entry(covered.file.as_str()).or_default();
                for line in &covered.lines {
                    lines.entry(*line).or_default().push(position);
                }
            }
        }
        LineIndex { map: self, by_file }
    }
}

impl TestCoverage {
    /// Test `id` (in `file`, when known) that executed `covered`: files merged and sorted, each
    /// file's lines ascending without repeats.
    pub fn new(id: &str, file: Option<String>, covered: Vec<CoveredLines>) -> Self {
        let mut by_file: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        for entry in covered {
            by_file.entry(entry.file).or_default().extend(entry.lines);
        }
        Self {
            id: id.to_owned(),
            file,
            covered: by_file
                .into_iter()
                .map(|(file, mut lines)| {
                    lines.sort_unstable();
                    lines.dedup();
                    CoveredLines { file, lines }
                })
                .collect(),
        }
    }
}

/// `line → tests` for every file of a [`PerTestMap`] (design §9.4).
pub struct LineIndex<'a> {
    map: &'a PerTestMap,
    by_file: BTreeMap<&'a str, BTreeMap<u32, Vec<usize>>>,
}

impl<'a> LineIndex<'a> {
    /// The tests that execute any line from `start` to `end` (inclusive) of `file`, in ID order,
    /// each with the first of those lines it executes.
    pub fn covering(&self, file: &str, start: u32, end: u32) -> Vec<(&'a TestCoverage, u32)> {
        let mut first: BTreeMap<usize, u32> = BTreeMap::new();
        if let Some(lines) = self.by_file.get(file) {
            for (line, tests) in lines.range(start..=end) {
                for test in tests {
                    first.entry(*test).or_insert(*line);
                }
            }
        }
        first
            .into_iter()
            .map(|(test, line)| (&self.map.tests[test], line))
            .collect()
    }
}
