// ABOUTME: Joint producers (ADR-084, ADR-149): one suite run per project root that makes `TestResults` and `CoverageMap`,
// ABOUTME: and `PerTestMap` where the tool attributes lines to tests; cached under one record, rerun per stale test on a range.
//! Joint suite runs.
//!
//! Where a language's runner and coverage tool are one toolchain, one run of the suite under
//! coverage reports every test's outcome and the coverage of the whole suite, and, where the
//! tool can attribute each line to the tests that ran it, the per-test map too. A joint producer
//! stands in for the per-kind producers that would each run the suite (`cupel_core::JointProducer`);
//! the fact store decides when it saves a run. Each root's green run is cached with the record of
//! the tree it describes and the toolchain that measured it, so an unchanged tree runs nothing; a
//! red or incomplete run is never cached, since what made it red may lie outside the tree (a full
//! disk), and runs again (ADR-141 §2). On a range scope below `deep`, a joint with a per-test map
//! reruns only the tests the change makes stale and keeps the rest from its entry (ADR-149).

mod go_test_cover;
mod llvm_cov_nextest;
mod pytest_cov_contexts;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use cupel_core::coverage::{CoverageMap, FileCoverage};
use cupel_core::syntax::{NormTree, Symbols, TestFrames};
use cupel_core::{
    Availability, Cancellation, CostClass, Ctx, Estimate, EventSink, Exclude, FactKind, Focus,
    JointProducer, LangId, Langs, PerTestMap, Process, ProduceCtx, Produced, ProducerId, Result,
    RunnerInvocation, Scope, SuiteMode, SuiteReuse, TestCoverage, TestResult, TestResults, ToolId,
    Upstream, case_name, find_program,
};
use serde::{Deserialize, Serialize};

use crate::fresh::{Sources, Tree, cache_dir, entries, entry_name, hash, load, skip_dirs, store};
use crate::per_test::cache::extension;
use crate::per_test::{
    Measured, PerTest, PerTestSpec, Site, cached_tests, edits, fresh_tests, merged, store_tests,
};
use crate::runner::{placed, roots, under};

/// The kind of cache the joint producers keep under `.cupel/cache/`.
const CACHE: &str = "suite";

/// Bumped when what a cache entry holds changes, so older entries are run again rather than read:
/// 2 since entries hold only green runs (ADR-141 §2), 3 since they record the toolchain and the
/// tests' structural hashes (ADR-149 §3–4).
const VERSION: u32 = 3;

/// The `[lang.<id>]` key holding the coverage command a joint must reproduce.
const COVERAGE: &str = "coverage";

/// One joint producer: everything that differs between Rust, Python, and Go.
pub(crate) struct SuiteSpec {
    /// The producer ID, such as `tool.llvm-cov-nextest`.
    pub id: &'static str,
    pub lang: &'static str,
    /// Each fact kind it makes, with the producer it stands in for.
    pub makes: &'static [(FactKind, &'static str)],
    /// The per-test producer whose cache entry it writes and whose staleness rule decides what
    /// an incremental run reruns (ADR-149 §1), when it makes the per-test map.
    pub per_test: Option<&'static PerTestSpec>,
    /// Every program a run needs.
    pub programs: &'static [&'static str],
    /// Directories the tools write below the root, left out of the record of the tree.
    pub outputs: &'static [&'static str],
    /// Whether the configured coverage command, by its words, is the one a run reproduces.
    pub fuses: fn(&[&str]) -> bool,
    /// The toolchain the suite runs under in a root (ADR-149 §4), recorded with its entry;
    /// `None` when it cannot be asked.
    pub toolchain: fn(&Path) -> Option<String>,
    /// One run of the suite in a root over every test but `kept`: all of them when it is empty.
    pub run: fn(&Site<'_>, &BTreeSet<String>) -> Result<SuiteRun>,
}

/// What one run of the suite in a root reported, relative to that root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SuiteRun {
    pub invocation: RunnerInvocation,
    pub tests: Vec<TestResult>,
    /// The whole run's coverage; `None` when the run made no report.
    pub coverage: Option<CoverageMap>,
    /// Each test's lines; `None` for a joint that does not make the per-test map, or a run that
    /// started no test.
    #[serde(skip)]
    pub per_test: Option<Vec<TestCoverage>>,
}

/// Per test ID, relative to the root, the structural hash of its definition (ADR-149 §3).
type Hashes = BTreeMap<String, String>;

/// What a cache entry's report holds.
#[derive(Debug, Serialize, Deserialize)]
struct Report {
    version: u32,
    /// The toolchain that measured the run (ADR-149 §4).
    toolchain: String,
    /// The tests' structural hashes, where the run knew them.
    hashes: Hashes,
    run: SuiteRun,
}

/// A cache entry read back: its record, when it was written, and its report, of this version
/// and toolchain.
struct Entry {
    sources: Sources,
    written: SystemTime,
    report: Report,
}

/// What an incremental run keeps from the entry and how many tests it reruns (ADR-149 §1).
struct Plan {
    /// The kept tests' coverage, their lines moved with the change.
    keep: Vec<TestCoverage>,
    /// How many cached tests are stale: 0 when the change touched no test and no source file,
    /// so the entry is served as it is.
    stale: usize,
    /// The entry's report: the recorded run (every test's result and the whole coverage) and
    /// the tests' structural hashes.
    recorded: Report,
}

/// A joint producer: `spec` run once per project root of its language.
pub(crate) struct Suite(pub &'static SuiteSpec);

impl JointProducer for Suite {
    fn id(&self) -> ProducerId {
        ProducerId::from(self.0.id)
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![LangId::from(self.0.lang)])
    }

    fn makes(&self) -> &[(FactKind, &'static str)] {
        self.0.makes
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    /// The functions of the files in scope, so a change reruns only the tests that ran a changed
    /// function (ADR-084 §7), and the tests' definitions, so a test whose own structure changed
    /// reruns (ADR-149 §3).
    fn wants(&self) -> &[FactKind] {
        &[FactKind::Symbols, FactKind::NormTree, FactKind::TestFrames]
    }

    /// One suite run, `standard`: `cached` when every root it holds an entry for is unchanged,
    /// `incremental: N stale` when every root that changed would rerun only its stale tests
    /// (ADR-149 §6), else `one run`.
    fn estimate(&self, ctx: &Ctx, upstream: &Upstream, focus: &Focus) -> Estimate {
        let entries = entries(&cache_dir(&ctx.root, CACHE, self.0.id));
        let mut cached = !entries.is_empty();
        let mut incremental = true;
        let mut stale = 0;
        for entry in &entries {
            let Some(found) = self.entry(entry, None) else {
                cached = false;
                incremental = false;
                continue;
            };
            let dir = found.sources.dir.clone();
            let root = ctx.root.join(&dir);
            if self.fresh(&ctx.root, &ctx.exclude, found, &dir).is_some() {
                continue;
            }
            cached = false;
            match self.plan(&ctx.root, &ctx.exclude, &dir, &root, entry, upstream, focus) {
                Some(plan) => stale += plan.stale,
                None => incremental = false,
            }
        }
        let note = if cached {
            "cached".to_owned()
        } else if incremental && !entries.is_empty() {
            format!("incremental: {stale} stale")
        } else {
            "one run".to_owned()
        };
        Estimate {
            cost: CostClass::Standard,
            note: Some(note),
        }
    }

    /// `Ready` with every program installed and the language's coverage command one a run
    /// reproduces; `unconfigured` for any other command, or none (ADR-084 §3).
    fn availability(&self, ctx: &Ctx) -> Availability {
        let command = ctx
            .lang
            .get(&LangId::from(self.0.lang))
            .and_then(|table| table.get(COVERAGE)?.as_str());
        let fuses = command
            .is_some_and(|command| (self.0.fuses)(&command.split_whitespace().collect::<Vec<_>>()));
        if !fuses {
            return Availability::Unconfigured {
                key: format!("lang.{}.{COVERAGE}", self.0.lang),
            };
        }
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

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<Vec<Produced>> {
        let mut made = Made::default();
        for (dir, packages) in roots(ctx) {
            let entry = ctx.writer.create_dir(
                &ctx.writer
                    .cache_dir()
                    .join(CACHE)
                    .join(self.0.id)
                    .join(entry_name(&dir)),
            )?;
            let path = ctx.ctx.root.join(&dir);
            let site = Site {
                ctx,
                dir: &dir,
                path: path.canonicalize().unwrap_or(path),
                packages,
                area: ctx.writer.create_dir(&entry.join("work"))?,
            };
            let repo = &ctx.ctx.root;
            let exclude = &ctx.ctx.exclude;
            let (run, mode, rerun) = match self.cached(repo, exclude, &entry, &dir) {
                Some(kept) => (kept.run, SuiteMode::Cached, 0),
                None => match self.plan(
                    repo,
                    exclude,
                    &dir,
                    &site.path,
                    &entry,
                    &ctx.upstream,
                    &ctx.focus,
                ) {
                    Some(plan) => self.incremental(&site, &entry, plan)?,
                    None => {
                        let run = self.full(&site, &entry)?;
                        let rerun = run.tests.len() as u64;
                        (run, SuiteMode::Full, rerun)
                    }
                },
            };
            made.add(&site, run, mode, rerun);
        }
        Ok(made.facts(self.0, &ctx.ctx.root))
    }
}

impl Suite {
    /// The toolchain of the root at `root`, asked once per process (ADR-149 §4); empty when it
    /// cannot be asked.
    fn toolchain(&self, root: &Path) -> String {
        static ASKED: Mutex<BTreeMap<(String, PathBuf), String>> = Mutex::new(BTreeMap::new());
        let key = (self.0.id.to_owned(), root.to_path_buf());
        let mut asked = ASKED.lock().unwrap_or_else(PoisonError::into_inner);
        asked
            .entry(key)
            .or_insert_with(|| (self.0.toolchain)(root).unwrap_or_default())
            .clone()
    }

    /// The entry at `entry` (for the root `dir`, when given), when its report reads back and is
    /// of this version.
    fn entry(&self, entry: &Path, dir: Option<&str>) -> Option<Entry> {
        let (sources, written, report) = load(entry)?;
        if dir.is_some_and(|dir| dir != sources.dir) {
            return None;
        }
        let report: Report = serde_json::from_slice(&report).ok()?;
        (report.version == VERSION).then_some(Entry {
            sources,
            written,
            report,
        })
    }

    /// `found` as the run to serve, when nothing under its root `dir` changed since it was
    /// written, this toolchain measured it, and the per-test entry it wrote is unchanged too.
    fn fresh(&self, repo: &Path, exclude: &Exclude, found: Entry, dir: &str) -> Option<Entry> {
        let root = repo.join(dir);
        if found.report.toolchain != self.toolchain(&root)
            || !found
                .sources
                .changes(repo, exclude, found.written)
                .is_empty()
        {
            return None;
        }
        let mut found = found;
        if let Some(spec) = self.0.per_test {
            found.report.run.per_test = Some(fresh_tests(repo, exclude, spec.id, dir)?);
        }
        Some(found)
    }

    /// The run cached in `entry` for the root `dir`, when it is fresh (see [`fresh`](Self::fresh)).
    fn cached(&self, repo: &Path, exclude: &Exclude, entry: &Path, dir: &str) -> Option<Report> {
        let found = self.entry(entry, Some(dir))?;
        Some(self.fresh(repo, exclude, found, dir)?.report)
    }

    /// What an incremental run in the root `dir` (at `root`) would keep and rerun (ADR-149 §1):
    /// `None` when the whole suite runs instead (§2): a tree scope, `deep`, no entry for this
    /// toolchain, no per-test entry, an added file that may hold a test, a change that makes
    /// every test stale, or a changed source file that no test ran.
    #[allow(clippy::too_many_arguments)]
    fn plan(
        &self,
        repo: &Path,
        exclude: &Exclude,
        dir: &str,
        root: &Path,
        entry: &Path,
        upstream: &Upstream,
        focus: &Focus,
    ) -> Option<Plan> {
        let spec = self.0.per_test?;
        if focus.changed.is_none() || focus.depth >= CostClass::Deep {
            return None;
        }
        let found = self.entry(entry, Some(dir))?;
        if found.report.toolchain != self.toolchain(root) {
            return None;
        }
        let cached = cached_tests(repo, spec.id, dir)?;
        let changes = cached.changes(repo, exclude);
        let is_source = |file: &String| spec.sources.contains(&extension(file));
        if changes.is_empty() || changes.added.iter().any(is_source) {
            return None;
        }
        let edits = edits(
            dir,
            root,
            &cached.lines,
            &changes,
            upstream.get::<Symbols>(),
        );
        let refresh = PerTest(spec)
            .staleness(root, edits)
            .refresh(cached.tests, &changes);
        let current = structural_hashes(
            found.report.run.tests.iter().map(|t| t.id.as_str()),
            upstream,
            dir,
        );
        let recorded = &found.report.hashes;
        let (restructured, keep): (Vec<TestCoverage>, Vec<TestCoverage>) =
            refresh.keep.into_iter().partition(|test| {
                match (current.get(&test.id), recorded.get(&test.id)) {
                    (Some((file, now)), Some(then)) => {
                        changes.changed.contains(file) && now != then
                    }
                    _ => false,
                }
            });
        let stale = refresh.stale + restructured.len();
        if keep.is_empty() || (stale == 0 && changes.all().any(is_source)) {
            return None;
        }
        Some(Plan {
            keep,
            stale,
            recorded: found.report,
        })
    }

    /// Runs the whole suite in the root of `site` and caches it in `entry` when it is whole.
    fn full(&self, site: &Site<'_>, entry: &Path) -> Result<SuiteRun> {
        let before = self.record(site);
        let run = (self.0.run)(site, &BTreeSet::new())?;
        let hashes = structural_hashes(
            run.tests.iter().map(|test| test.id.as_str()),
            &site.ctx.upstream,
            site.dir,
        )
        .into_iter()
        .map(|(id, (_, hash))| (id, hash))
        .collect();
        self.keep(site, entry, &run, hashes, before)?;
        Ok(run)
    }

    /// Runs the tests `plan` does not keep, merges the run with what it keeps (ADR-149 §1), and
    /// caches the merged run in `entry` when it is whole; the run, how it was obtained, and how
    /// many tests ran. A plan with nothing stale serves the recorded run and renews its record.
    fn incremental(
        &self,
        site: &Site<'_>,
        entry: &Path,
        plan: Plan,
    ) -> Result<(SuiteRun, SuiteMode, u64)> {
        let before = self.record(site);
        let kept: BTreeSet<String> = plan.keep.iter().map(|test| test.id.clone()).collect();
        if plan.stale == 0 {
            let mut run = plan.recorded.run;
            run.per_test = Some(plan.keep);
            self.keep(site, entry, &run, plan.recorded.hashes, before)?;
            return Ok((run, SuiteMode::Cached, 0));
        }
        let measured = (self.0.run)(site, &kept)?;
        if measured.invocation.completed && measured.per_test.is_none() {
            // The stale tests are gone: nothing ran, so nothing was reported.
            let run = self.full(site, entry)?;
            let rerun = run.tests.len() as u64;
            return Ok((run, SuiteMode::Full, rerun));
        }
        let rerun = measured.tests.len() as u64;
        let mut hashes = plan.recorded.hashes.clone();
        hashes.extend(
            structural_hashes(
                measured
                    .tests
                    .iter()
                    .map(|test| test.id.as_str())
                    .chain(kept.iter().map(String::as_str)),
                &site.ctx.upstream,
                site.dir,
            )
            .into_iter()
            .map(|(id, (_, hash))| (id, hash)),
        );
        let run = merge(measured, plan.recorded.run, plan.keep);
        self.keep(site, entry, &run, hashes, before)?;
        Ok((run, SuiteMode::Incremental, rerun))
    }

    /// The record of the tree of `site` now.
    fn record(&self, site: &Site<'_>) -> Sources {
        let ctx = site.ctx;
        let skip = skip_dirs(ctx, site.dir, self.0.outputs);
        let tree = Tree {
            repo: &ctx.ctx.root,
            dir: site.dir,
            skip_dirs: &skip,
            exclude: &ctx.ctx.exclude,
        };
        Sources::record(&tree, &site.path, &[])
    }

    /// Caches `run` in `entry` with the record of the tree after it, unless it did not complete,
    /// made a fact short, or had a test fail or error (ADR-141 §2); `before` is the record made
    /// before it ran, so a file edited meanwhile counts as changed.
    fn keep(
        &self,
        site: &Site<'_>,
        entry: &Path,
        run: &SuiteRun,
        hashes: Hashes,
        before: Sources,
    ) -> Result<()> {
        let whole = run.invocation.completed
            && !run.tests.iter().any(|test| test.outcome.is_failure())
            && run.coverage.is_some()
            && (self.0.per_test.is_none() || run.per_test.is_some());
        if !whole {
            return Ok(());
        }
        let mut sources = self.record(site).since(&before);
        if let (Some(spec), Some(tests)) = (self.0.per_test, &run.per_test) {
            store_tests(site, spec.id, tests, sources.clone())?;
        }
        let report = serde_json::to_vec(&Report {
            version: VERSION,
            toolchain: self.toolchain(&site.path),
            hashes,
            run: run.clone(),
        })
        .unwrap_or_default();
        sources.report = hash(&report);
        store(&site.ctx.writer, entry, &report, &sources)
    }
}

/// `measured`, the run over the tests not in `keep`, merged with what `recorded` holds of the
/// kept tests (ADR-149 §1): the kept tests' recorded results beside the measured ones (a
/// measured test replaces a kept one of the same ID), the kept tests' lines beside the measured
/// tests', and the measured run's report with one hit for every kept test on each line it
/// executed that the report instruments.
fn merge(measured: SuiteRun, recorded: SuiteRun, keep: Vec<TestCoverage>) -> SuiteRun {
    let ran: BTreeSet<&str> = measured.tests.iter().map(|test| test.id.as_str()).collect();
    let kept: BTreeSet<&str> = keep.iter().map(|test| test.id.as_str()).collect();
    let mut tests: Vec<TestResult> = recorded
        .tests
        .into_iter()
        .filter(|test| kept.contains(test.id.as_str()) && !ran.contains(test.id.as_str()))
        .collect();
    tests.extend(measured.tests);
    let coverage = measured.coverage.map(|report| with_kept(report, &keep));
    let per_test = measured.per_test.map(|found| {
        merged(
            keep,
            Measured {
                tests: found,
                listed: None,
            },
        )
    });
    SuiteRun {
        invocation: measured.invocation,
        tests,
        coverage,
        per_test,
    }
}

/// `report` with one hit added for every test of `kept` on each line it executed that the
/// report instruments (ADR-149 §1); a line the report does not instrument no longer exists.
fn with_kept(report: CoverageMap, kept: &[TestCoverage]) -> CoverageMap {
    let instrumented: BTreeMap<&str, BTreeSet<u32>> = report
        .files
        .iter()
        .map(|file| {
            (
                file.file.as_str(),
                file.lines.iter().map(|hits| hits.line).collect(),
            )
        })
        .collect();
    let mut records: Vec<FileCoverage> = Vec::new();
    for test in kept {
        for covered in &test.covered {
            let Some(lines) = instrumented.get(covered.file.as_str()) else {
                continue;
            };
            let hits: Vec<(u32, u64)> = covered
                .lines
                .iter()
                .filter(|line| lines.contains(line))
                .map(|line| (*line, 1))
                .collect();
            if !hits.is_empty() {
                records.push(FileCoverage::from_hits(&covered.file, &hits, None));
            }
        }
    }
    let mut files = report.files;
    files.extend(records);
    CoverageMap::new(files)
}

/// Per test of `ids` (relative to the root `dir`), the file of its test case relative to the
/// root and the structural hash of the innermost function holding the case (ADR-149 §3), where
/// `upstream`'s `TestFrames` name exactly one case of the test's name (the ID's last part, as
/// `tests --impacted` reads it) and its `Symbols` and `NormTree` hold that function.
fn structural_hashes<'a>(
    ids: impl Iterator<Item = &'a str>,
    upstream: &Upstream,
    dir: &str,
) -> BTreeMap<String, (String, String)> {
    let (Some(frames), Some(symbols), Some(norm)) = (
        upstream.get::<TestFrames>(),
        upstream.get::<Symbols>(),
        upstream.get::<NormTree>(),
    ) else {
        return BTreeMap::new();
    };
    let mut cases: BTreeMap<&str, Vec<(&str, u32, u32)>> = BTreeMap::new();
    for file in &frames.files {
        for case in &file.cases {
            cases.entry(case.name.as_str()).or_default().push((
                file.file.as_str(),
                case.span.line,
                case.span.end_line,
            ));
        }
    }
    ids.filter_map(|id| {
        let [(file, line, end_line)] = cases.get(case_name(id))?.as_slice() else {
            return None;
        };
        let holder = symbols
            .symbols
            .iter()
            .filter(|symbol| {
                symbol.file == *file
                    && symbol.span.line <= *line
                    && *end_line <= symbol.span.end_line
            })
            .min_by_key(|symbol| symbol.span.end_line - symbol.span.line)?;
        let key = holder.key();
        let hash = norm
            .symbols
            .iter()
            .find(|symbol| symbol.symbol == key)?
            .hash
            .to_string();
        Some((id.to_owned(), (in_root(dir, file)?, hash)))
    })
    .collect()
}

/// `path` (relative to the repository root) relative to the project root `dir`; `None` outside it.
fn in_root(dir: &str, path: &str) -> Option<String> {
    if dir == "." {
        Some(path.to_owned())
    } else {
        path.strip_prefix(&format!("{dir}/")).map(str::to_owned)
    }
}

/// The standard output of `program` with `args` in `root`, trimmed, when it exits 0.
pub(crate) fn probe(root: &Path, program: &str, args: &[&str]) -> Option<String> {
    let captured = Process::new(program, root)
        .args(args.iter().copied())
        .run(&Cancellation::new())
        .ok()?;
    (captured.exit_code == Some(0)).then(|| captured.stdout.trim().to_owned())
}

/// The facts of every root so far, relative to the repository root.
#[derive(Default)]
struct Made {
    invocations: Vec<RunnerInvocation>,
    tests: Vec<TestResult>,
    /// `None` once a root made no coverage.
    coverage: Option<Vec<FileCoverage>>,
    /// `None` once a root made no per-test map.
    per_test: Option<Vec<TestCoverage>>,
    /// How many tests ran, and how many were kept from the cache, over every root.
    rerun: u64,
    reused: u64,
    /// How each root's run was obtained.
    modes: BTreeSet<SuiteMode>,
    roots: usize,
}

impl Made {
    fn add(&mut self, site: &Site<'_>, run: SuiteRun, mode: SuiteMode, rerun: u64) {
        let first = self.roots == 0;
        self.roots += 1;
        self.modes.insert(mode);
        self.rerun += rerun;
        self.reused += (run.tests.len() as u64).saturating_sub(rerun);
        self.invocations.push(run.invocation);
        self.tests
            .extend(run.tests.into_iter().map(|test| placed(test, site.dir)));
        self.coverage = run.coverage.and_then(|map| {
            let mut files = if first {
                Some(Vec::new())
            } else {
                self.coverage.take()
            }?;
            files.extend(map.files.into_iter().map(|mut file| {
                file.file = under(site.dir, &file.file, "/");
                file
            }));
            Some(files)
        });
        self.per_test = run.per_test.and_then(|tests| {
            let mut all = if first {
                Some(Vec::new())
            } else {
                self.per_test.take()
            }?;
            all.extend(tests.into_iter().map(|test| site.placed(test)));
            Some(all)
        });
    }

    /// How the roots' runs were obtained together: as every one was, when they agree, else
    /// incremental.
    fn mode(&self) -> SuiteMode {
        match self.modes.iter().collect::<Vec<_>>().as_slice() {
            [one] => **one,
            _ => SuiteMode::Incremental,
        }
    }

    /// The facts `spec` makes from what every root made; the coverage and per-test map only
    /// when every root made them.
    fn facts(self, spec: &SuiteSpec, repo: &Path) -> Vec<Produced> {
        let runner = |kind: FactKind| {
            spec.makes
                .iter()
                .find(|(made, _)| *made == kind)
                .map(|(_, id)| ProducerId::from(*id))
        };
        let Some(results_runner) = runner(FactKind::TestResults) else {
            return Vec::new();
        };
        let reuse = SuiteReuse {
            mode: self.mode(),
            rerun: self.rerun,
            reused: self.reused,
        };
        let mut facts = vec![Produced::new(
            TestResults::new(results_runner.clone(), self.invocations, self.tests, repo)
                .with_reuse(reuse),
        )];
        if let Some(files) = self.coverage {
            facts.push(Produced::new(CoverageMap::new(files)));
        }
        if let (Some(tests), Some(_)) = (self.per_test, spec.per_test) {
            facts.push(Produced::new(PerTestMap::refreshed(
                results_runner,
                tests,
                self.rerun,
            )));
        }
        facts
    }
}
