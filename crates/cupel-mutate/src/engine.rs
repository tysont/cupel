// ABOUTME: `engine.mutation` (ADR-080, ADR-082, ADR-150): plans the mutants of `MutationSites` in focus, reuses stored
// ABOUTME: outcomes, and runs the rest file by file in overlays, as schemata or file copies, timing each phase.
use std::path::Path;
use std::time::Instant;

use cupel_core::coverage::CoverageMap;
use cupel_core::syntax::{
    ImportTarget, ModuleGraph, MutationSites, NormTree, Symbols, TestConventions, TestFrames,
};
use cupel_core::{
    Availability, CostClass, Ctx, Error, EventSink, ExcludedTest, FactKind, Langs,
    MutationOutcomes, MutationPhase, PerTestMap, Process, ProduceCtx, Producer, ProducerId,
    ProducerRegistry, Result, Runner, SLOW_TEST_MS, Scope, TestResults, TestSelection, WriteError,
};

use crate::execute::{Execution, FileRun};
use crate::history::History;
use crate::overlay::Slot;
use crate::phases::{Phases, Served};
use crate::plan::{Inputs, Planned, digest, plan};
use crate::schemata::{self, Rewriter};
use crate::settings::{Settings, Strategy};
use crate::slots::{Claim, claim, slot_dir};

/// The engine's producer ID.
pub const ID: &str = "engine.mutation";

/// Bumped when the engine changes how it runs a mutant, so stored outcomes are not reused.
const ENGINE_VERSION: &str = "1";

/// The mutation engine (ADR-080).
pub struct MutationEngine;

/// Adds the engine to `registry`.
pub fn register(registry: &mut ProducerRegistry) {
    registry.register::<MutationOutcomes, _>(MutationEngine);
}

impl Producer<MutationOutcomes> for MutationEngine {
    fn id(&self) -> ProducerId {
        ProducerId::from(ID)
    }

    fn languages(&self) -> Langs {
        Langs::Any
    }

    fn needs(&self) -> &[FactKind] {
        &[
            FactKind::MutationSites,
            FactKind::Symbols,
            FactKind::CoverageMap,
            FactKind::TestResults,
        ]
    }

    fn wants(&self) -> &[FactKind] {
        &[
            FactKind::PerTestMap,
            FactKind::ModuleGraph,
            FactKind::NormTree,
            FactKind::ProjectInfo,
            FactKind::TestConventions,
            FactKind::TestFrames,
        ]
    }

    fn cost(&self) -> CostClass {
        CostClass::Standard
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        _: &mut dyn EventSink,
    ) -> Result<MutationOutcomes> {
        let upstream = &ctx.upstream;
        let (Some(baseline), Some(sites), Some(symbols), Some(coverage)) = (
            upstream.get::<TestResults>(),
            upstream.get::<MutationSites>(),
            upstream.get::<Symbols>(),
            upstream.get::<CoverageMap>(),
        ) else {
            return Ok(self.outcomes(ctx, None, false, Vec::new()));
        };
        let settings = Settings::read(&ctx.config, &ctx.settings)?;
        let runner = ctx
            .runners
            .get(&baseline.runner)
            .ok_or_else(|| Error::InvalidValue {
                what: "runner for engine.mutation",
                value: baseline.runner.to_string(),
                expected: "a registered runner",
            })?;
        let per_test = upstream
            .get::<PerTestMap>()
            .filter(|map| map.runner == baseline.runner);
        let toolchain = toolchain_hash(ctx, runner);
        let excluded = excluded(ctx, runner, baseline);
        let whole = whole_suite(&excluded);
        let suite_hash = whole_suite_hash(baseline, &excluded);
        let inputs = Inputs {
            sites,
            symbols,
            coverage,
            norm: upstream.get::<NormTree>(),
            conventions: upstream.get::<TestConventions>(),
            frames: upstream.get::<TestFrames>(),
            per_test,
            focus: &ctx.focus,
            scope,
            operators: settings.operators,
            suite_hash: &suite_hash,
            toolchain_hash: &toolchain,
        };
        let root = &ctx.ctx.root;
        let planning = Instant::now();
        let mut planned = plan(&inputs, &|file| read(root, file));
        let phases = Phases::default();
        Running::new(ctx, runner, settings, symbols, &whole, &phases)
            .run(&mut planned, planning)?;
        let runs_whole = planned
            .iter()
            .any(|planned| planned.covered && planned.tests.is_none());
        let mutants = planned.into_iter().map(|planned| planned.mutant).collect();
        let mut outcomes = self.outcomes(ctx, Some(baseline), per_test.is_some(), mutants);
        if runs_whole {
            outcomes.excluded = excluded;
        }
        outcomes.phases = phases.rows();
        Ok(outcomes)
    }
}

impl MutationEngine {
    fn outcomes(
        &self,
        ctx: &ProduceCtx,
        baseline: Option<&TestResults>,
        per_test: bool,
        mutants: Vec<cupel_core::Mutant>,
    ) -> MutationOutcomes {
        MutationOutcomes::new(
            self.id(),
            baseline.map(|baseline| baseline.runner.clone()),
            ctx.focus.depth,
            per_test,
            mutants,
        )
    }
}

/// The tests of `baseline` slower than [`SLOW_TEST_MS`], when `runner` can leave them out of a
/// run of the whole suite (ADR-147); none otherwise, so every mutant runs them.
fn excluded(ctx: &ProduceCtx, runner: &dyn Runner, baseline: &TestResults) -> Vec<ExcludedTest> {
    let slow: Vec<ExcludedTest> = baseline
        .tests
        .iter()
        .filter(|test| test.duration_ms.is_some_and(|ms| ms > SLOW_TEST_MS))
        .map(|test| ExcludedTest {
            id: test.id.clone(),
            file: test.file.clone(),
            baseline_ms: test.duration_ms.unwrap_or_default(),
        })
        .collect();
    let ids = slow.iter().map(|test| test.id.clone()).collect();
    if slow.is_empty() || runner.commands(ctx, &TestSelection::Except(ids)).is_none() {
        return Vec::new();
    }
    slow
}

/// What a mutant that per-test selection did not narrow runs: the whole suite but `excluded`.
fn whole_suite(excluded: &[ExcludedTest]) -> TestSelection {
    if excluded.is_empty() {
        TestSelection::All
    } else {
        TestSelection::Except(excluded.iter().map(|test| test.id.clone()).collect())
    }
}

/// The suite hash of a mutant that runs the whole suite: the baseline's, with the IDs of the
/// tests left out of it when any is, so a test becoming slow or fast reruns those mutants.
fn whole_suite_hash(baseline: &TestResults, excluded: &[ExcludedTest]) -> String {
    if excluded.is_empty() {
        return baseline.suite_hash.clone();
    }
    let mut parts = vec![baseline.suite_hash.as_str()];
    parts.extend(excluded.iter().map(|test| test.id.as_str()));
    digest(&parts)
}

/// One production's running of its planned mutants.
struct Running<'a> {
    ctx: &'a ProduceCtx,
    runner: &'a dyn Runner,
    settings: Settings,
    /// What a mutant that per-test selection did not narrow runs.
    whole: &'a TestSelection,
    graph: Option<&'a ModuleGraph>,
    /// The language's schemata rewriter, when its mutants are built as schemata.
    rewriter: Option<&'static dyn Rewriter>,
    /// The language being produced, which names its slots.
    lang: String,
    /// Where every phase of the production is recorded (ADR-150).
    phases: &'a Phases,
}

impl<'a> Running<'a> {
    /// How `ctx`'s production of `symbols`' language runs its mutants with `runner`, those
    /// per-test selection did not narrow over `whole`, recording its phases in `phases`.
    fn new(
        ctx: &'a ProduceCtx,
        runner: &'a dyn Runner,
        settings: Settings,
        symbols: &Symbols,
        whole: &'a TestSelection,
        phases: &'a Phases,
    ) -> Self {
        let lang = symbols
            .symbols
            .first()
            .map_or_else(String::new, |symbol| symbol.lang.to_string());
        Self {
            ctx,
            runner,
            settings,
            whole,
            graph: ctx.upstream.get::<ModuleGraph>(),
            rewriter: match settings.strategy {
                Strategy::Schemata => schemata::rewriter(&lang),
                Strategy::FileCopy => None,
            },
            lang,
            phases,
        }
    }

    /// Reuses what the store holds and runs the other covered mutants, file by file. The plan
    /// phase started at `planning` and ends with the history read; a production with nothing
    /// to run records no phase.
    fn run(&self, planned: &mut [Planned], planning: Instant) -> Result<()> {
        let history = History::load(&self.ctx.writer, planned);
        let mut to_run = Vec::new();
        for (index, mutant) in planned.iter_mut().enumerate() {
            if mutant.covered && !history.reuse(mutant) {
                to_run.push(index);
            }
        }
        if to_run.is_empty() {
            return Ok(());
        }
        self.phases
            .record(MutationPhase::Plan, Served::default(), planning);
        let order = history.order(planned, &to_run);
        let widest = order.iter().map(|(_, indexes)| indexes.len()).max();
        let (_claim, slots) = self.slots(widest.unwrap_or(1))?;
        let execution = Execution {
            ctx: self.ctx,
            runner: self.runner,
            slots: &slots,
            settings: self.settings,
            whole: self.whole,
            phases: self.phases,
        };
        let root = &self.ctx.ctx.root;
        for (file, indexes) in order {
            self.ctx.cancel.checkpoint()?;
            let original =
                std::fs::read(root.join(&file)).map_err(|error| io(root, &file, &error))?;
            let importers = self.importers(&file);
            let run = FileRun {
                file: &file,
                original: &original,
                importers: &importers,
                mutants: indexes.iter().map(|index| &planned[*index]).collect(),
            };
            let found = match schemata::run_file(&execution, self.rewriter, &run) {
                Ok(found) => found,
                Err(error) => {
                    execution.restore(&run);
                    return Err(error);
                }
            };
            for (index, ran) in indexes.iter().zip(found) {
                let mutant = &mut planned[*index].mutant;
                mutant.outcome = ran.outcome;
                mutant.tests = ran.tests;
                mutant.killing_tests = ran.killing_tests;
                mutant.duration_ms = Some(ran.duration_ms);
                mutant.strategy = Some(ran.strategy);
                mutant.fallback = ran.fallback;
            }
        }
        Ok(())
    }

    /// The slots for files of at most `widest` mutants, each synced with the tree: as many as
    /// the settings allow, no more than the run's `--jobs` (ADR-086 §11); held by the claim
    /// until the production ends (ADR-141 §3).
    fn slots(&self, widest: usize) -> Result<(Claim, Vec<Slot>)> {
        let root = &self.ctx.ctx.root;
        let claim = claim(root).map_err(|error| io(root, "", &error))?;
        let jobs = self
            .ctx
            .focus
            .jobs
            .map_or(self.settings.jobs, |cap| self.settings.jobs.min(cap.get()));
        let slots = (0..jobs.min(widest))
            .map(|index| {
                let slot = Slot::at(slot_dir(root, &self.lang, index));
                let started = Instant::now();
                slot.sync(root, self.runner.outputs())
                    .map_err(|error| io(slot.root(), "", &error))?;
                let served = Served {
                    slot: Some(index),
                    ..Served::default()
                };
                self.phases.record(MutationPhase::Sync, served, started);
                Ok(slot)
            })
            .collect::<Result<_>>()?;
        Ok((claim, slots))
    }

    /// Each file that imports `file` by a relative path, with its text (ADR-080 §8).
    fn importers(&self, file: &str) -> Vec<(String, Vec<u8>)> {
        let root = &self.ctx.ctx.root;
        let mut importers: Vec<String> = self
            .graph
            .into_iter()
            .flat_map(|graph| &graph.roots)
            .flat_map(|imports| &imports.edges)
            .filter(|edge| edge.relative)
            .filter(|edge| matches!(&edge.target, ImportTarget::File { path } if path == file))
            .map(|edge| edge.from.clone())
            .collect();
        importers.sort();
        importers.dedup();
        importers
            .into_iter()
            .filter_map(|importer| {
                Some((importer.clone(), std::fs::read(root.join(&importer)).ok()?))
            })
            .collect()
    }
}

/// `file`'s text under `root`.
fn read(root: &Path, file: &str) -> Option<String> {
    std::fs::read_to_string(root.join(file)).ok()
}

/// BLAKE3 of the engine's version, the runner, and the first line its tool's version command
/// prints (ADR-080 §10).
fn toolchain_hash(ctx: &ProduceCtx, runner: &dyn Runner) -> String {
    let version = runner
        .tool(&ctx.ctx)
        .and_then(|probe| {
            let (program, args) = probe.version.split_first()?;
            let captured = Process::new(program, &ctx.ctx.root)
                .args(args.iter().cloned())
                .run(&ctx.cancel)
                .ok()?;
            captured.stdout.lines().next().map(str::to_owned)
        })
        .unwrap_or_default();
    digest(&[ENGINE_VERSION, runner.id().as_str(), &version])
}

fn io(root: &Path, file: &str, error: &std::io::Error) -> Error {
    Error::Write(WriteError::Io {
        path: root.join(file),
        message: error.to_string(),
    })
}
