// ABOUTME: The `PerTestMap` producers (ADR-077, ADR-083): what they share (a spec per tool, run per project root and
// ABOUTME: cached there under `.cupel/`, refreshed by re-measuring only stale tests), and one test's executed lines.
//! Per-test coverage producers.
//!
//! Each producer lists or runs a root's tests under a coverage tool so that every report it reads
//! belongs to one test, and maps the report's executed lines to the runner's test IDs. The
//! reports are read by the `CoverageMap` format parsers (`crate::coverage`), and
//! [`executed_lines`] keeps the lines each test ran. Each root's map is cached with the record of
//! the tree it measured; a later production keeps the tests no change touched and measures only
//! the rest (ADR-083).

mod cache;
mod go_cover;
mod llvm_cov;
mod pytest_cov;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use cupel_core::coverage::CoverageMap;
use cupel_core::{
    Availability, Captured, CostClass, CoveredLines, Ctx, Error, Estimate, EventSink, FactKind,
    Langs, PerTestMap, Process, ProduceCtx, Producer, ProducerId, Result, Scope, TestCoverage,
    ToolId, ToolProbe, Upstream, find_program, output_tail,
};

use self::cache::{Cached, Staleness, report};
use crate::fresh::{Sources, Tree, cache_dir, entries, entry_name, hash, skip_dirs, store};
use crate::runner::{ID_SEPARATOR, roots, under};

/// The kind of cache the producers keep under `.cupel/cache/`.
const CACHE: &str = "per-test";

/// The most stale tests a producer that runs each test on its own refreshes at `standard`
/// (ADR-083); more, and refreshing costs what measuring does, `deep`.
const REFRESH_LIMIT: usize = 32;

/// One per-test producer: everything that differs between Rust, Go, and Python.
pub(crate) struct PerTestSpec {
    /// The producer ID, such as `tool.llvm-cov-per-test`.
    pub id: &'static str,
    pub languages: fn() -> Langs,
    /// The `TestResults` producer whose IDs the map uses.
    pub runner: &'static str,
    /// Every program the producer runs; the first is the one `doctor` probes.
    pub programs: &'static [&'static str],
    /// The command that prints the first program's version.
    pub version: &'static [&'static str],
    /// How many runs measuring takes.
    pub runs: Runs,
    /// Directories the tools write below the root, left out of the record of the tree.
    pub outputs: &'static [&'static str],
    /// File names whose change re-measures every test (build and test configuration).
    pub whole: &'static [&'static str],
    /// Extensions of files that may hold a test.
    pub sources: &'static [&'static str],
    /// Whether the file (relative to the root at the path) is part of the test itself.
    pub owns: fn(&Path, &TestCoverage, &str) -> bool,
    /// The tests of one root, but those whose IDs it is given, and the lines each executed,
    /// relative to that root.
    pub measure: fn(&Site<'_>, &BTreeSet<String>) -> Result<Measured>,
}

/// How many tool runs measuring a root takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Runs {
    /// One run of the suite, however many tests it measures.
    Suite,
    /// One run per test.
    PerTest,
}

/// What measuring a root found.
pub(crate) struct Measured {
    /// The tests measured, relative to the root.
    pub tests: Vec<TestCoverage>,
    /// Every test the root holds, when the producer listed them; a cached test not among them
    /// is gone.
    pub listed: Option<BTreeSet<String>>,
}

/// A per-test producer: `spec` run once per project root of its language.
pub(crate) struct PerTest(pub &'static PerTestSpec);

/// A project root being measured.
pub(crate) struct Site<'a> {
    pub ctx: &'a ProduceCtx,
    /// The root relative to the repository root; `.` for the root itself.
    pub dir: &'a str,
    /// Absolute and canonical.
    pub path: PathBuf,
    /// Package patterns from `ProjectInfo`.
    pub packages: Vec<String>,
    /// A directory under `.cupel/` for this root's reports.
    pub area: PathBuf,
}

impl Producer<PerTestMap> for PerTest {
    fn id(&self) -> ProducerId {
        ProducerId::from(self.0.id)
    }

    fn languages(&self) -> Langs {
        (self.0.languages)()
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    fn cost(&self) -> CostClass {
        CostClass::Deep
    }

    /// `standard` when every root has a cache entry and refreshing them is cheap: always for
    /// a producer that measures in one suite run, at most [`REFRESH_LIMIT`] stale tests for one
    /// that runs each test alone; else `deep` (ADR-083).
    fn estimate(&self, ctx: &Ctx, _upstream: &Upstream) -> Estimate {
        let mut stale = 0;
        let mut measure = false;
        let mut found = false;
        for entry in entries(&cache_dir(&ctx.root, CACHE, self.0.id)) {
            let Some(cached) = Cached::read(&entry) else {
                return Estimate::of(self.cost());
            };
            let changes = cached.changes(&ctx.root, &ctx.exclude);
            let root = ctx.root.join(&cached.sources.dir);
            let refresh = self.staleness(&root).refresh(cached.tests, &changes);
            stale += refresh.stale;
            measure |= refresh.measure;
            found = true;
        }
        if !found {
            return Estimate::of(self.cost());
        }
        let cheap = self.0.runs == Runs::Suite || stale <= REFRESH_LIMIT;
        Estimate {
            cost: if cheap {
                CostClass::Standard
            } else {
                self.cost()
            },
            note: Some(if measure {
                format!("refresh {stale} stale")
            } else {
                "cached".to_owned()
            }),
        }
    }

    fn availability(&self, _: &Ctx) -> Availability {
        match self
            .0
            .programs
            .iter()
            .find(|program| find_program(program).is_none())
        {
            Some(missing) => Availability::MissingTool {
                name: ToolId::from(*missing),
                install: None,
            },
            None => Availability::Ready,
        }
    }

    fn tool(&self, _: &Ctx) -> Option<ToolProbe> {
        Some(ToolProbe {
            name: ToolId::from(self.0.programs[0]),
            version: self
                .0
                .version
                .iter()
                .map(|word| (*word).to_owned())
                .collect(),
        })
    }

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<PerTestMap> {
        let mut tests = Vec::new();
        let mut measured = 0;
        for (dir, packages) in roots(ctx) {
            let path = ctx.ctx.root.join(&dir);
            let entry = ctx.writer.create_dir(
                &ctx.writer
                    .cache_dir()
                    .join(CACHE)
                    .join(self.0.id)
                    .join(entry_name(&dir)),
            )?;
            let site = Site {
                ctx,
                dir: &dir,
                path: path.canonicalize().unwrap_or(path),
                packages,
                area: ctx.writer.create_dir(&entry.join("work"))?,
            };
            let (found, count) = self.root(&site, &entry)?;
            measured += count;
            tests.extend(found.into_iter().map(|test| site.placed(test)));
        }
        Ok(PerTestMap::refreshed(
            ProducerId::from(self.0.runner),
            tests,
            measured,
        ))
    }
}

impl PerTest {
    /// The tests of `site`'s root, relative to it, and how many were measured: the cache entry
    /// `entry` when nothing changed, else its tests no change touched and the rest measured,
    /// cached again with the record of the tree after measuring.
    fn root(&self, site: &Site<'_>, entry: &Path) -> Result<(Vec<TestCoverage>, u64)> {
        let ctx = site.ctx;
        let skip = skip_dirs(ctx, site.dir, self.0.outputs);
        let tree = Tree {
            repo: &ctx.ctx.root,
            dir: site.dir,
            skip_dirs: &skip,
            exclude: &ctx.ctx.exclude,
        };
        let before = Sources::record(&tree, &site.path, &[]);
        let (keep, measure) = match Cached::load(entry, site.dir) {
            Some(cached) => {
                let changes = cached.changes(&ctx.ctx.root, &ctx.ctx.exclude);
                if changes.is_empty() {
                    return Ok((cached.tests, 0));
                }
                let refresh = self.staleness(&site.path).refresh(cached.tests, &changes);
                (refresh.keep, refresh.measure)
            }
            None => (Vec::new(), true),
        };
        let (tests, measured, mut sources) = if measure {
            let kept: BTreeSet<String> = keep.iter().map(|test| test.id.clone()).collect();
            let found = (self.0.measure)(site, &kept)?;
            let measured = found.tests.len() as u64;
            // The tools may write files (`Cargo.lock`); a file edited while they ran is stale.
            let after = Sources::record(&tree, &site.path, &[]).since(&before);
            (merged(keep, found), measured, after)
        } else {
            (keep, 0, before)
        };
        let report = report(&tests);
        sources.report = hash(&report);
        store(&ctx.writer, entry, &report, &sources)?;
        Ok((tests, measured))
    }

    /// What makes a cached test of the root at `root` stale for this producer.
    fn staleness<'a>(&self, root: &'a Path) -> Staleness<'a> {
        let owns = self.0.owns;
        Staleness {
            whole: self.0.whole,
            sources: self.0.sources,
            owns: Box::new(move |test: &TestCoverage, file: &str| owns(root, test, file)),
        }
    }
}

/// The kept tests that are still listed, with the measured ones, which replace any kept test of
/// the same ID.
fn merged(keep: Vec<TestCoverage>, found: Measured) -> Vec<TestCoverage> {
    let measured: BTreeSet<&str> = found.tests.iter().map(|test| test.id.as_str()).collect();
    let mut tests: Vec<TestCoverage> = keep
        .into_iter()
        .filter(|test| {
            !measured.contains(test.id.as_str())
                && found
                    .listed
                    .as_ref()
                    .is_none_or(|listed| listed.contains(&test.id))
        })
        .collect();
    tests.extend(found.tests);
    tests
}

impl Site<'_> {
    /// Runs `program` with `args` in the root, with `env` added, under the run's cancellation.
    pub fn run(&self, program: &str, args: &[String], env: &[(&str, &str)]) -> Result<Captured> {
        let mut process = Process::new(program, &self.path).args(args.iter().cloned());
        for (key, value) in env {
            process = process.env(key, value);
        }
        process.run(&self.ctx.cancel)
    }

    /// Runs `program` with `args` like [`run`](Self::run), failing unless it exits 0.
    pub fn succeed(
        &self,
        program: &str,
        args: &[String],
        env: &[(&str, &str)],
    ) -> Result<Captured> {
        let captured = self.run(program, args, env)?;
        if captured.exit_code == Some(0) {
            Ok(captured)
        } else {
            Err(failed(program, &captured))
        }
    }

    /// A report file in the root's area.
    pub fn report(&self, name: &str) -> PathBuf {
        self.area.join(name)
    }

    /// `path`, as a report names it (absolute, or relative to the root), relative to the root;
    /// `None` for a file outside it.
    pub fn file(&self, path: &str) -> Option<String> {
        let path = Path::new(path);
        let relative = if path.is_absolute() {
            path.strip_prefix(&self.path).ok()?
        } else {
            path
        };
        Some(relative.to_string_lossy().replace('\\', "/"))
    }

    /// `test`, measured relative to this root, made relative to the repository root.
    fn placed(&self, test: TestCoverage) -> TestCoverage {
        TestCoverage::new(
            &under(self.dir, &test.id, ID_SEPARATOR),
            test.file.map(|file| under(self.dir, &file, "/")),
            test.covered
                .into_iter()
                .map(|covered| CoveredLines {
                    file: under(self.dir, &covered.file, "/"),
                    lines: covered.lines,
                })
                .collect(),
        )
    }
}

/// The error of `program` that did not finish its work, with the tail of its output.
pub(crate) fn failed(program: &str, captured: &Captured) -> Error {
    Error::Process {
        program: program.to_owned(),
        message: output_tail(&format!("{}\n{}", captured.stdout, captured.stderr))
            .unwrap_or_else(|| format!("exited with {:?}", captured.exit_code)),
    }
}

/// Lines per file as `CoveredLines`, keeping the files `file` maps into the repository.
pub(crate) fn covered(
    lines: BTreeMap<String, Vec<u32>>,
    file: impl Fn(&str) -> Option<String>,
) -> Vec<CoveredLines> {
    lines
        .into_iter()
        .filter_map(|(path, lines)| {
            Some(CoveredLines {
                file: file(&path)?,
                lines,
            })
        })
        .collect()
}

/// The executed lines (hits above zero) per file of one test's coverage, files as `map` names
/// them.
pub(crate) fn executed_lines(map: &CoverageMap) -> BTreeMap<String, Vec<u32>> {
    map.files
        .iter()
        .map(|file| {
            let lines: Vec<u32> = file
                .lines
                .iter()
                .filter(|hits| hits.hits > 0)
                .map(|hits| hits.line)
                .collect();
            (file.file.clone(), lines)
        })
        .filter(|(_, lines)| !lines.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;

    use super::executed_lines;
    use crate::coverage::{parse_coverprofile, parse_lcov, placed};

    fn lines(files: &std::collections::BTreeMap<String, Vec<u32>>) -> Vec<(String, Vec<u32>)> {
        files
            .iter()
            .map(|(file, lines)| (file.clone(), lines.clone()))
            .collect()
    }

    #[test]
    fn one_tests_lcov_report_yields_the_lines_it_executed() {
        let capture = Capture::of("per-test/cargo-llvm-cov", "adds");
        let map = parse_lcov(&capture.file("report.lcov")).unwrap();
        assert_eq!(
            lines(&executed_lines(&placed(map, &[&capture.ran_in], "."))),
            [("src/lib.rs".to_owned(), vec![4, 5, 6, 18, 19, 20])]
        );
    }

    #[test]
    fn one_tests_coverprofile_yields_the_lines_of_its_executed_blocks() {
        let read = |test: &str| {
            let profile = Capture::of("per-test/go-cover", test).file("coverprofile.out");
            lines(&executed_lines(&parse_coverprofile(&profile).unwrap()))
        };
        assert_eq!(
            read("TestAdd"),
            [("example.com/calc/calc.go".to_owned(), vec![5, 6, 7])]
        );
        assert_eq!(
            read("TestScale"),
            [("example.com/calc/calc.go".to_owned(), vec![10, 11, 12])]
        );
    }
}
