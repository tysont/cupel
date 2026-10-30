// ABOUTME: The per-test map cache (ADR-083, ADR-084): one entry per producer and project root under `.cupel/cache/per-test/`,
// ABOUTME: and the refresh of a stale entry, which keeps every test no changed function touches and re-measures the rest.
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::SystemTime;

use cupel_core::{Attribution, CoveredLines, Exclude, TestCoverage};
use serde::{Deserialize, Serialize};

use super::functions::FunctionEdit;
use crate::fresh::{Changes, Sources, load};
use crate::runner::{ID_SEPARATOR, under};

/// Bumped when what a cache entry holds, or what its tests mean, changes, so older entries are
/// measured again rather than read.
const VERSION: u32 = 2;

/// Per file, a hash of each line as measured (`functions::line_hashes`).
pub(crate) type Lines = BTreeMap<String, Vec<u64>>;

/// What a cache entry's report holds: the root's tests and the lines of every file they cover
/// as measured, relative to the root.
#[derive(Debug, Serialize, Deserialize)]
struct Report {
    version: u32,
    tests: Vec<TestCoverage>,
    lines: Lines,
}

/// A cache entry read back: its record, when it was written, its tests, and the lines they
/// were measured on.
pub(crate) struct Cached {
    pub sources: Sources,
    pub written: SystemTime,
    /// Relative to the project root, as they were measured.
    pub tests: Vec<TestCoverage>,
    pub lines: Lines,
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
            lines: report.lines,
        })
    }

    /// The files under the root that changed since the entry was written.
    pub fn changes(&self, repo: &Path, exclude: &Exclude) -> Changes {
        self.sources.changes(repo, exclude, self.written)
    }
}

/// The report of a cache entry holding `tests`, measured on `lines`.
pub(crate) fn report(tests: &[TestCoverage], lines: Lines) -> Vec<u8> {
    let report = Report {
        version: VERSION,
        tests: tests.to_vec(),
        lines,
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
    /// Per changed file, what the change did to its functions (ADR-084 §7); a changed file not
    /// here is stale for every test that covers it.
    pub edits: BTreeMap<String, FunctionEdit>,
    /// The root below the repository root (`.` for the root itself), which the tests' IDs and
    /// the changed files are relative to.
    pub dir: &'a str,
    /// The language's attributions (ADR-151); none at `--depth deep`.
    pub attributions: Vec<Attribution>,
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
    /// The kept tests their attribution kept though they ran a changed function (ADR-151 §4).
    pub tagged: BTreeSet<String>,
    /// Whether to measure at all: a test is stale, or a changed file may hold a new test.
    pub measure: bool,
}

impl Staleness<'_> {
    /// The refresh of `tests` after `changes`: every test is stale when a `whole` file changed;
    /// otherwise a test is stale when it owns a changed, added, or removed file, and, as its
    /// attribution says when it has one (ADR-151 §2), when a changed, added, or removed file is
    /// one its attribution names, or else when it ran a changed function of a changed file (any
    /// line of a changed file without an edit, or a removed one). The rest are kept, their lines
    /// moved with the edits.
    pub fn refresh(&self, tests: Vec<TestCoverage>, changes: &Changes) -> Refresh {
        let whole = changes.all().any(|file| self.whole.contains(&name(file)));
        let mut keep = Vec::new();
        let mut stale = 0;
        let mut tagged = BTreeSet::new();
        for test in tests {
            let ran = self.ran_a_change(&test.covered, &changes.changed);
            let attributed = self.attributed(&test.id);
            let named = || {
                changes.all().any(|file| {
                    let file = under(self.dir, file, "/");
                    attributed.iter().any(|found| found.covers(&file))
                })
            };
            let owns = changes.all().any(|file| (self.owns)(&test, file));
            let is_stale = whole || owns || if attributed.is_empty() { ran } else { named() };
            if is_stale {
                stale += 1;
                continue;
            }
            if ran {
                tagged.insert(test.id.clone());
            }
            keep.push(self.moved(test));
        }
        let new_tests = changes
            .all()
            .any(|file| self.sources.iter().any(|ext| extension(file) == *ext));
        Refresh {
            keep,
            stale,
            tagged,
            measure: stale > 0 || new_tests,
        }
    }

    /// The attributions that name the test `id` (relative to the root).
    fn attributed(&self, id: &str) -> Vec<&Attribution> {
        if self.attributions.is_empty() {
            return Vec::new();
        }
        let id = under(self.dir, id, ID_SEPARATOR);
        self.attributions
            .iter()
            .filter(|attribution| attribution.names(&id))
            .collect()
    }

    /// Whether `covered` runs a changed function of one of the changed `files`, or any line of
    /// one without an edit.
    fn ran_a_change(&self, covered: &[CoveredLines], files: &BTreeSet<String>) -> bool {
        covered.iter().any(|lines| {
            files.contains(&lines.file)
                && self
                    .edits
                    .get(&lines.file)
                    .is_none_or(|edit| edit.touches(&lines.lines))
        })
    }

    /// `test` with its lines in edited files numbered as they are now.
    fn moved(&self, test: TestCoverage) -> TestCoverage {
        let covered = test
            .covered
            .into_iter()
            .map(|lines| match self.edits.get(&lines.file) {
                Some(edit) => CoveredLines {
                    lines: edit.moved(&lines.lines),
                    file: lines.file,
                },
                None => lines,
            })
            .collect();
        TestCoverage::new(&test.id, test.file, covered)
    }
}

/// Whether `file` is the test's own file, when the producer names it.
pub(crate) fn own_file(test: &TestCoverage, file: &str) -> bool {
    test.file.as_deref() == Some(file)
}

/// The last part of a `/`-separated path.
fn name(file: &str) -> &str {
    file.rsplit('/').next().unwrap_or(file)
}

/// The extension of a `/`-separated path, without the dot; empty without one.
pub(crate) fn extension(file: &str) -> &str {
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
            edits: BTreeMap::new(),
            dir: ".",
            attributions: Vec::new(),
        };
        let refresh = staleness.refresh(cached(), changes);
        let kept = refresh.keep.into_iter().map(|test| test.id).collect();
        (kept, refresh.stale, refresh.measure)
    }

    const ADD: &str = "tests/test_add.py::test_add";
    const SCALE: &str = "tests/test_scale.py::test_scale";

    #[test]
    fn a_changed_source_or_test_file_makes_only_the_tests_covering_or_owning_it_stale() {
        let source = refreshed(&changes(&[], &["add.py"]));
        let test_file = refreshed(&changes(&[], &["tests/test_scale.py"]));
        assert_eq!(
            [source, test_file],
            [
                (vec![SCALE.to_owned()], 1, true),
                (vec![ADD.to_owned()], 1, true)
            ]
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

    /// The kept IDs, the tagged ones, and how many are stale after `changes`, with the
    /// attribution `binary`: `tests/test_scale.py::*`, attributed to `cli.py` and `data/`.
    fn attributed(changes: &Changes) -> (Vec<String>, Vec<String>, usize) {
        let serde_json::Value::Object(table) = serde_json::json!({"attribution": {"binary": {
            "tests": ["tests/test_scale.py::*"], "files": ["cli.py", "data"],
        }}}) else {
            unreachable!()
        };
        let staleness = Staleness {
            whole: &["conftest.py"],
            sources: &["py"],
            owns: Box::new(|_: &TestCoverage, _: &str| false),
            edits: BTreeMap::new(),
            dir: ".",
            attributions: cupel_core::attributions(&"python".into(), &table).unwrap(),
        };
        let refresh = staleness.refresh(cached(), changes);
        let kept = refresh.keep.into_iter().map(|test| test.id).collect();
        (kept, refresh.tagged.into_iter().collect(), refresh.stale)
    }

    #[test]
    fn an_attributed_test_is_stale_only_for_a_change_to_its_attributions_files() {
        assert_eq!(
            [
                attributed(&changes(&[], &["scale.py"])),
                attributed(&changes(&[], &["data/pairs.txt"])),
                attributed(&changes(&[], &["add.py"])),
                attributed(&changes(&[], &["conftest.py"])),
            ],
            [
                (
                    vec![ADD.to_owned(), SCALE.to_owned()],
                    vec![SCALE.to_owned()],
                    0
                ),
                (vec![ADD.to_owned()], Vec::new(), 1),
                (vec![SCALE.to_owned()], Vec::new(), 1),
                (Vec::new(), Vec::new(), 2),
            ]
        );
    }
}
