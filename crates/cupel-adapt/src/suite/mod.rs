// ABOUTME: Joint producers (ADR-084): one suite run per project root that makes `TestResults` and `CoverageMap`, and
// ABOUTME: `PerTestMap` where the tool attributes lines to tests, cached together under one freshness record.
//! Joint suite runs.
//!
//! Where a language's runner and coverage tool are one toolchain, one run of the suite under
//! coverage reports every test's outcome and the coverage of the whole suite, and, where the
//! tool can attribute each line to the tests that ran it, the per-test map too. A joint producer
//! stands in for the per-kind producers that would each run the suite (`cupel_core::JointProducer`);
//! the fact store decides when it saves a run. Each root's green run is cached with the record of
//! the tree it describes, so an unchanged tree runs nothing; a red or incomplete run is never
//! cached, since what made it red may lie outside the tree (a full disk), and runs again
//! (ADR-141 §2).

mod go_test_cover;
mod llvm_cov_nextest;
mod pytest_cov_contexts;

use std::path::Path;

use cupel_core::coverage::{CoverageMap, FileCoverage};
use cupel_core::{
    Availability, CostClass, Ctx, Estimate, EventSink, Exclude, FactKind, Focus, JointProducer,
    LangId, Langs, PerTestMap, ProduceCtx, Produced, ProducerId, Result, RunnerInvocation, Scope,
    TestCoverage, TestResult, TestResults, ToolId, Upstream, find_program,
};
use serde::{Deserialize, Serialize};

use crate::fresh::{Sources, Tree, cache_dir, entries, entry_name, hash, load, skip_dirs, store};
use crate::per_test::{Site, fresh_tests, store_tests};
use crate::runner::{placed, roots, under};

/// The kind of cache the joint producers keep under `.cupel/cache/`.
const CACHE: &str = "suite";

/// Bumped when what a cache entry holds changes, so older entries are run again rather than read:
/// 2 since entries hold only green runs (ADR-141 §2).
const VERSION: u32 = 2;

/// The `[lang.<id>]` key holding the coverage command a joint must reproduce.
const COVERAGE: &str = "coverage";

/// One joint producer: everything that differs between Rust, Python, and Go.
pub(crate) struct SuiteSpec {
    /// The producer ID, such as `tool.llvm-cov-nextest`.
    pub id: &'static str,
    pub lang: &'static str,
    /// Each fact kind it makes, with the producer it stands in for.
    pub makes: &'static [(FactKind, &'static str)],
    /// The per-test producer whose cache entry it writes, when it makes the per-test map.
    pub per_test: Option<&'static str>,
    /// Every program a run needs.
    pub programs: &'static [&'static str],
    /// Directories the tools write below the root, left out of the record of the tree.
    pub outputs: &'static [&'static str],
    /// Whether the configured coverage command, by its words, is the one a run reproduces.
    pub fuses: fn(&[&str]) -> bool,
    /// One run of the suite in a root.
    pub run: fn(&Site<'_>) -> Result<SuiteRun>,
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

/// What a cache entry's report holds.
#[derive(Debug, Serialize, Deserialize)]
struct Report {
    version: u32,
    run: SuiteRun,
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

    /// One suite run, `standard`; `cached` when every root it holds an entry for is unchanged.
    fn estimate(&self, ctx: &Ctx, _: &Upstream, _: &Focus) -> Estimate {
        let entries = entries(&cache_dir(&ctx.root, CACHE, self.0.id));
        let cached = !entries.is_empty()
            && entries
                .iter()
                .all(|entry| self.cached(&ctx.root, &ctx.exclude, entry, None).is_some());
        Estimate {
            cost: CostClass::Standard,
            note: Some(if cached { "cached" } else { "one run" }.to_owned()),
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
            let cached = self.cached(&ctx.ctx.root, &ctx.ctx.exclude, &entry, Some(&dir));
            let fresh = cached.is_some();
            let run = match cached {
                Some(run) => run,
                None => self.run(&site, &entry)?,
            };
            made.add(&site, run, fresh);
        }
        Ok(made.facts(self.0, &ctx.ctx.root))
    }
}

impl Suite {
    /// The run cached in `entry` (for the root `dir`, when given), when its report reads back,
    /// nothing under the root changed since, and the per-test entry it wrote is unchanged too.
    fn cached(
        &self,
        repo: &Path,
        exclude: &Exclude,
        entry: &Path,
        dir: Option<&str>,
    ) -> Option<SuiteRun> {
        let (sources, written, report) = load(entry)?;
        if dir.is_some_and(|dir| dir != sources.dir)
            || !sources.fresh(repo, exclude, written, &report)
        {
            return None;
        }
        let report: Report = serde_json::from_slice(&report).ok()?;
        if report.version != VERSION {
            return None;
        }
        let mut run = report.run;
        if let Some(id) = self.0.per_test {
            run.per_test = Some(fresh_tests(repo, exclude, id, &sources.dir)?);
        }
        Some(run)
    }

    /// Runs the suite in the root of `site` and caches what it made in `entry` with the record
    /// of the tree after the run, unless the run did not complete, made a fact short, or had a
    /// test fail or error.
    fn run(&self, site: &Site<'_>, entry: &Path) -> Result<SuiteRun> {
        let ctx = site.ctx;
        let skip = skip_dirs(ctx, site.dir, self.0.outputs);
        let tree = Tree {
            repo: &ctx.ctx.root,
            dir: site.dir,
            skip_dirs: &skip,
            exclude: &ctx.ctx.exclude,
        };
        let before = Sources::record(&tree, &site.path, &[]);
        let run = (self.0.run)(site)?;
        let whole = run.invocation.completed
            && !run.tests.iter().any(|test| test.outcome.is_failure())
            && run.coverage.is_some()
            && (self.0.per_test.is_none() || run.per_test.is_some());
        if whole {
            let mut sources = Sources::record(&tree, &site.path, &[]).since(&before);
            if let (Some(id), Some(tests)) = (self.0.per_test, &run.per_test) {
                store_tests(site, id, tests, sources.clone())?;
            }
            let report = serde_json::to_vec(&Report {
                version: VERSION,
                run: run.clone(),
            })
            .unwrap_or_default();
            sources.report = hash(&report);
            store(&ctx.writer, entry, &report, &sources)?;
        }
        Ok(run)
    }
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
    /// Whether every root came from the cache.
    cached: bool,
    roots: usize,
}

impl Made {
    fn add(&mut self, site: &Site<'_>, run: SuiteRun, cached: bool) {
        let first = self.roots == 0;
        self.roots += 1;
        self.cached = (first || self.cached) && cached;
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
        let mut facts = vec![Produced::new(TestResults::new(
            results_runner.clone(),
            self.invocations,
            self.tests,
            repo,
        ))];
        if let Some(files) = self.coverage {
            facts.push(Produced::new(CoverageMap::new(files)));
        }
        if let (Some(tests), Some(_)) = (self.per_test, spec.per_test) {
            let measured = if self.cached { 0 } else { tests.len() as u64 };
            facts.push(Produced::new(PerTestMap::refreshed(
                results_runner,
                tests,
                measured,
            )));
        }
        facts
    }
}
