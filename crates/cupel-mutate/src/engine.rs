// ABOUTME: `engine.mutation` (ADR-080, ADR-082): plans the mutants of `MutationSites` in focus, reuses stored outcomes,
// ABOUTME: and runs the rest file by file in overlays, as schemata or file copies, into `MutationOutcomes`.
use std::path::Path;

use cupel_core::coverage::CoverageMap;
use cupel_core::syntax::{
    ImportTarget, ModuleGraph, MutationSites, NormTree, Symbols, TestConventions, TestFrames,
};
use cupel_core::{
    Availability, CostClass, Ctx, Error, EventSink, FactKind, Langs, MutationOutcomes, PerTestMap,
    Process, ProduceCtx, Producer, ProducerId, ProducerRegistry, Result, Runner, Scope,
    TestResults, WriteError,
};

use crate::execute::{Execution, FileRun};
use crate::history::History;
use crate::overlay::{Slot, claim, slot_dir};
use crate::plan::{Inputs, Planned, digest, plan};
use crate::schemata::{self, Rewriter};
use crate::settings::{Settings, Strategy};

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
        let settings = Settings::read(&ctx.config)?;
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
            suite_hash: &baseline.suite_hash,
            toolchain_hash: &toolchain,
        };
        let root = &ctx.ctx.root;
        let mut planned = plan(&inputs, &|file| read(root, file));
        let lang = symbols
            .symbols
            .first()
            .map_or_else(String::new, |symbol| symbol.lang.to_string());
        let running = Running {
            ctx,
            runner,
            settings,
            graph: upstream.get::<ModuleGraph>(),
            rewriter: match settings.strategy {
                Strategy::Schemata => schemata::rewriter(&lang),
                Strategy::FileCopy => None,
            },
            lang,
        };
        running.run(&mut planned)?;
        let mutants = planned.into_iter().map(|planned| planned.mutant).collect();
        Ok(self.outcomes(ctx, Some(baseline), per_test.is_some(), mutants))
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

/// One production's running of its planned mutants.
struct Running<'a> {
    ctx: &'a ProduceCtx,
    runner: &'a dyn Runner,
    settings: Settings,
    graph: Option<&'a ModuleGraph>,
    /// The language's schemata rewriter, when its mutants are built as schemata.
    rewriter: Option<&'static dyn Rewriter>,
    /// The language being produced, which names its slots.
    lang: String,
}

impl Running<'_> {
    /// Reuses what the store holds and runs the other covered mutants, file by file.
    fn run(&self, planned: &mut [Planned]) -> Result<()> {
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
        let order = history.order(planned, &to_run);
        let widest = order.iter().map(|(_, indexes)| indexes.len()).max();
        let slots = self.slots(widest.unwrap_or(1))?;
        let execution = Execution {
            ctx: self.ctx,
            runner: self.runner,
            slots: &slots,
            timeout_factor: self.settings.timeout_factor,
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
            let found = schemata::run_file(&execution, self.rewriter, &run)?;
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

    /// The slots for files of at most `widest` mutants, each synced with the tree.
    fn slots(&self, widest: usize) -> Result<Vec<Slot>> {
        let root = &self.ctx.ctx.root;
        claim(root).map_err(|error| io(root, "", &error))?;
        (0..self.settings.jobs.min(widest))
            .map(|index| {
                let slot = Slot::at(slot_dir(root, &self.lang, index));
                slot.sync(root, self.runner.outputs())
                    .map_err(|error| io(slot.root(), "", &error))?;
                Ok(slot)
            })
            .collect()
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
