// ABOUTME: The per-test map cache (ADR-083): one entry per producer and project root under `.cupel/cache/per-test/`,
// ABOUTME: and the refresh of a stale entry, which keeps every test no changed file touches and re-measures the rest.
use std::collections::BTreeSet;
use std::path::Path;
use std::time::SystemTime;

use cupel_core::{CoveredLines, Exclude, TestCoverage};
use serde::{Deserialize, Serialize};

use crate::fresh::{Changes, Sources, load};

/// Bumped when what a cache entry holds, or what its tests mean, changes, so older entries are
/// measured again rather than read.
const VERSION: u32 = 1;

/// What a cache entry's report holds: the root's tests, relative to the root.
#[derive(Debug, Serialize, Deserialize)]
struct Report {
    version: u32,
    tests: Vec<TestCoverage>,
}

/// A cache entry read back: its record, when it was written, and its tests.
pub(crate) struct Cached {
    pub sources: Sources,
    pub written: SystemTime,
    /// Relative to the project root, as they were measured.
    pub tests: Vec<TestCoverage>,
}

impl Cached {
    /// The entry at `entry` for the project root `dir`; `None` when there is none, it is for
    /// another root, its report is not the one recorded, or it is of another version.
    pub fn load(entry: &Path, dir: &str) -> Option<Self> {
        Self::read(entry).filter(|cached| cached.sources.dir == dir)
    }

    /// The entry at `entry`, for whichever root it records.
    pub fn read(entry: &Path) -> Option<Self> {
        let (sources, written, report) = load(entry)?;
        let report: Report = serde_json::from_slice(&report).ok()?;
        (report.version == VERSION).then_some(Self {
            sources,
            written,
            tests: report.tests,
        })
    }

    /// The files under the root that changed since the entry was written.
    pub fn changes(&self, repo: &Path, exclude: &Exclude) -> Changes {
        self.sources.changes(repo, exclude, self.written)
    }
}

/// The report of a cache entry holding `tests`.
pub(crate) fn report(tests: &[TestCoverage]) -> Vec<u8> {
    let report = Report {
        version: VERSION,
        tests: tests.to_vec(),
    };
    serde_json::to_vec(&report).unwrap_or_default()
}

/// What decides which cached tests a change makes stale, per producer.
pub(crate) struct Staleness<'a> {
    /// File names any change to which re-measures every test: build and test configuration.
    pub whole: &'a [&'a str],
    /// Extensions of files that may hold tests: a change to one may add a test.
    pub sources: &'a [&'a str],
    /// Whether `file` (relative to the root) is part of `test` itself, beyond the files it covers.
    pub owns: Owns<'a>,
}

/// Whether a file (relative to the root) is part of a test itself.
pub(crate) type Owns<'a> = Box<dyn Fn(&TestCoverage, &str) -> bool + 'a>;

/// How a cache entry is brought up to date.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Refresh {
    /// The cached tests no change touches, kept without measuring them again.
    pub keep: Vec<TestCoverage>,
    /// How many cached tests must be measured again.
    pub stale: usize,
    /// Whether to measure at all: a test is stale, or a changed file may hold a new test.
    pub measure: bool,
}

impl Staleness<'_> {
    /// The refresh of `tests` after `changes`: every test is stale when a `whole` file changed;
    /// otherwise a test is stale when it covers a changed or removed file or owns a changed,
    /// added, or removed one, and the rest are kept.
    pub fn refresh(&self, tests: Vec<TestCoverage>, changes: &Changes) -> Refresh {
        let whole = changes.all().any(|file| self.whole.contains(&name(file)));
        let (stale, keep): (Vec<TestCoverage>, Vec<TestCoverage>) =
            tests.into_iter().partition(|test| {
                whole
                    || covers_any(&test.covered, &changes.changed)
                    || changes.all().any(|file| (self.owns)(test, file))
            });
        let new_tests = changes
            .all()
            .any(|file| self.sources.iter().any(|ext| extension(file) == *ext));
        Refresh {
            keep,
            stale: stale.len(),
            measure: !stale.is_empty() || new_tests,
        }
    }
}

/// Whether any of `covered` is one of `files`.
fn covers_any(covered: &[CoveredLines], files: &BTreeSet<String>) -> bool {
    covered.iter().any(|lines| files.contains(&lines.file))
}

/// Whether `file` is the test's own file, when the producer names it.
pub(crate) fn own_file(test: &TestCoverage, file: &str) -> bool {
    test.file.as_deref() == Some(file)
}

/// The last part of a `/`-separated path.
fn name(file: &str) -> &str {
    file.rsplit('/').next().unwrap_or(file)
}

fn extension(file: &str) -> &str {
    name(file).rsplit_once('.').map_or("", |(_, ext)| ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test(id: &str, file: Option<&str>, covered: &[&str]) -> TestCoverage {
        TestCoverage::new(
            id,
            file.map(str::to_owned),
            covered
                .iter()
                .map(|file| CoveredLines {
                    file: (*file).to_owned(),
                    lines: vec![1],
                })
                .collect(),
        )
    }

    /// Two tests in their own files, each covering its own source file.
    fn cached() -> Vec<TestCoverage> {
        vec![
            test(
                "tests/test_add.py::test_add",
                Some("tests/test_add.py"),
                &["add.py", "tests/test_add.py"],
            ),
            test(
                "tests/test_scale.py::test_scale",
                Some("tests/test_scale.py"),
                &["scale.py", "tests/test_scale.py"],
            ),
        ]
    }

    fn changes(added: &[&str], changed: &[&str]) -> Changes {
        Changes {
            added: added.iter().map(|file| (*file).to_owned()).collect(),
            changed: changed.iter().map(|file| (*file).to_owned()).collect(),
        }
    }

    /// The IDs kept, how many are stale, and whether to measure, after `changes`.
    fn refreshed(changes: &Changes) -> (Vec<String>, usize, bool) {
        let staleness = Staleness {
            whole: &["conftest.py"],
            sources: &["py"],
            owns: Box::new(own_file),
        };
        let refresh = staleness.refresh(cached(), changes);
        let kept = refresh.keep.into_iter().map(|test| test.id).collect();
        (kept, refresh.stale, refresh.measure)
    }

    const ADD: &str = "tests/test_add.py::test_add";
    const SCALE: &str = "tests/test_scale.py::test_scale";

    #[test]
    fn a_changed_source_makes_only_the_tests_covering_it_stale() {
        assert_eq!(
            refreshed(&changes(&[], &["add.py"])),
            (vec![SCALE.to_owned()], 1, true)
        );
    }

    #[test]
    fn a_changed_or_removed_test_file_makes_its_own_tests_stale() {
        assert_eq!(
            refreshed(&changes(&[], &["tests/test_scale.py"])),
            (vec![ADD.to_owned()], 1, true)
        );
    }

    #[test]
    fn an_added_source_file_keeps_every_test_but_is_measured_for_new_tests() {
        assert_eq!(
            refreshed(&changes(&["tests/test_new.py"], &[])),
            (vec![ADD.to_owned(), SCALE.to_owned()], 0, true)
        );
    }

    #[test]
    fn a_change_no_test_covers_in_a_file_that_holds_no_test_measures_nothing() {
        assert_eq!(
            refreshed(&changes(&["README.md"], &["notes.txt"])),
            (vec![ADD.to_owned(), SCALE.to_owned()], 0, false)
        );
    }

    #[test]
    fn a_change_to_a_whole_suite_file_anywhere_makes_every_test_stale() {
        assert_eq!(
            refreshed(&changes(&[], &["tests/conftest.py"])),
            (Vec::new(), 2, true)
        );
    }
}
