// ABOUTME: A mutation production cut short by the run's cancellation (ADR-172): the mutants that finished keep their
// ABOUTME: outcomes, those the cut left are `not-run`, and the slots hold the original file; skipped without pytest.
mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use cupel_core::coverage::CoverageMap;
use cupel_core::syntax::{MutationSites, ProjectInfo, Symbols};
use cupel_core::{
    Availability, Cancellation, CostClass, Ctx, Estimate, EventSink, FactKind, Langs,
    MutationOutcomes, ProduceCtx, Producer, ProducerId, ProducerRegistry, Result, RunOptions,
    RunOrdering, Runner, RunnerCommand, Scope, TestResults, TestRun, TestSelection, ToolProbe,
    Upstream, Writer,
};
use support::{Run, registries, summary};

/// The fixture's runner, which cancels the run just before its `cut`-th test run: that run and
/// every later one end as a cut run does.
#[derive(Clone)]
struct Cutting {
    runner: &'static dyn Runner,
    cancel: Cancellation,
    runs: Arc<AtomicUsize>,
    cut: usize,
}

impl Producer<TestResults> for Cutting {
    fn id(&self) -> ProducerId {
        self.runner.id()
    }

    fn languages(&self) -> Langs {
        self.runner.languages()
    }

    fn needs(&self) -> &[FactKind] {
        self.runner.needs()
    }

    fn cost(&self) -> CostClass {
        self.runner.cost()
    }

    fn estimate(&self, ctx: &Ctx, upstream: &Upstream) -> Estimate {
        self.runner.estimate(ctx, upstream)
    }

    fn availability(&self, ctx: &Ctx) -> Availability {
        self.runner.availability(ctx)
    }

    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe> {
        self.runner.tool(ctx)
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        sink: &mut dyn EventSink,
    ) -> Result<TestResults> {
        self.runner.produce(ctx, scope, sink)
    }
}

impl Runner for Cutting {
    fn list(&self, ctx: &ProduceCtx) -> Result<Vec<String>> {
        self.runner.list(ctx)
    }

    fn run(
        &self,
        ctx: &ProduceCtx,
        selection: &TestSelection,
        options: &RunOptions,
    ) -> Result<TestRun> {
        if self.runs.fetch_add(1, Ordering::SeqCst) + 1 == self.cut {
            self.cancel.cancel();
        }
        self.runner.run(ctx, selection, options)
    }

    fn commands(&self, ctx: &ProduceCtx, selection: &TestSelection) -> Option<Vec<RunnerCommand>> {
        self.runner.commands(ctx, selection)
    }

    fn ordering(&self) -> RunOrdering {
        self.runner.ordering()
    }

    fn outputs(&self) -> &'static [&'static str] {
        self.runner.outputs()
    }
}

/// The Python fixture's mutants, produced in one slot by a runner that cancels the run before
/// its `cut`-th test run (the first is the file's reference run).
fn cut_at(dir: &std::path::Path, cut: usize) -> Result<MutationOutcomes> {
    let mut upstream = Upstream::default();
    support::with_production(dir, "python", &Run::default(), |production, lang| {
        for kind in [
            FactKind::TestResults,
            FactKind::MutationSites,
            FactKind::Symbols,
            FactKind::CoverageMap,
        ] {
            upstream.insert(production.ensure(kind, lang, &mut Vec::new()).unwrap());
        }
        for kind in [FactKind::TestConventions, FactKind::ModuleGraph] {
            if let Ok(fact) = production.ensure(kind, lang, &mut Vec::new()) {
                upstream.insert(fact);
            }
        }
    });
    assert!(upstream.get::<MutationSites>().is_some() && upstream.get::<Symbols>().is_some());
    assert!(upstream.get::<CoverageMap>().is_some());
    let baseline = upstream.get::<TestResults>().unwrap();
    let cancel = Cancellation::new();
    let mut runners = ProducerRegistry::default();
    runners.register_runner(Cutting {
        runner: registries().producers.runner(&baseline.runner).unwrap(),
        cancel: cancel.clone(),
        runs: Arc::new(AtomicUsize::new(0)),
        cut,
    });
    let mut ctx = ProduceCtx::new(
        Ctx::new(dir.to_path_buf()),
        Writer::new(dir, false).unwrap().confined(),
        cancel,
    );
    ctx.upstream = upstream;
    ctx.runners = runners.runners();
    ctx.focus.jobs = std::num::NonZeroUsize::new(1);
    let files = cupel_lang::walk(dir, &registries().languages, &ProjectInfo::default());
    cupel_mutate::MutationEngine.produce(&ctx, &Scope::of_files(files), &mut Vec::new())
}

#[test]
fn a_cut_production_keeps_the_mutants_that_finished_and_marks_the_rest_not_run() {
    if !support::installed("pytest", "the Python mutation fixture") {
        return;
    }
    let dir = support::fixture("tests.mutation/python");
    // The reference run, then two mutants; the cut lands as the third mutant's run starts.
    let outcomes = cut_at(dir.path(), 4).expect("a cut production returns what it finished");
    assert_eq!(
        summary(&outcomes),
        [
            "6 >=→> survived",
            "6 and→or killed",
            "7 -→+ not-run",
            "8 *→/ not-run (equivalent?)",
            "8 1→0 not-run",
            "13 -→+ uncovered",
        ]
    );
    let not_run: Vec<(usize, Option<u64>, Option<_>)> = outcomes
        .mutants
        .iter()
        .filter(|mutant| mutant.outcome.as_str() == "not-run")
        .map(|mutant| (mutant.tests.len(), mutant.duration_ms, mutant.strategy))
        .collect();
    assert_eq!(not_run, [(0, None, None); 3], "a mutant not run has no run");
    let original = std::fs::read_to_string(dir.path().join("discount.py")).unwrap();
    let slot = cupel_mutate::slots::slot_dir(dir.path(), "python", 0);
    assert_eq!(
        std::fs::read_to_string(slot.join("discount.py")).unwrap(),
        original,
        "the slot holds the original file"
    );
}

#[test]
fn a_production_cut_before_any_mutant_ran_marks_every_covered_mutant_not_run() {
    if !support::installed("pytest", "the Python mutation fixture") {
        return;
    }
    let dir = support::fixture("tests.mutation/python");
    // The cut lands as the reference run starts.
    let outcomes = cut_at(dir.path(), 1).expect("a cut production returns what it finished");
    assert_eq!(
        summary(&outcomes),
        [
            "6 >=→> not-run",
            "6 and→or not-run",
            "7 -→+ not-run",
            "8 *→/ not-run (equivalent?)",
            "8 1→0 not-run",
            "13 -→+ uncovered",
        ]
    );
}
