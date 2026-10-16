// ABOUTME: The scheduler under `--budget` and `--jobs` over synthetic registries (ADR-086): closures that do not fit
// ABOUTME: are skipped with `est_ms`, history changes the choice, the deadline cuts a running task, durations are kept.
mod support;

use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cupel_core::budget::{Budget, History, Resources, TaskKind, TaskShape};
use cupel_core::config::Config;
use cupel_core::syntax::{
    Decisions, ModuleGraph, Span, SymbolDef, SymbolKind, Symbols, TestFrames, Visibility,
};
use cupel_core::{
    Availability, Check, CheckCtx, CheckId, CheckRegistry, CheckResult, CheckStatus, CostClass,
    Ctx, Error, Event, EventSink, Fact, FactKind, Facts, LangId, Langs, Outcome, Plan, ProduceCtx,
    Producer, ProducerId, ProducerRegistry, Result, Scheduler, Scope, SkipReason, Tier, Writer,
};

/// Makes a default `F` for rust after waiting `wait` on the run's cancellation, counting its
/// productions and remembering the concurrency cap it was handed.
struct Slow<F> {
    id: &'static str,
    cost: CostClass,
    wait: Duration,
    made: Arc<AtomicUsize>,
    jobs: Arc<Mutex<Vec<Option<NonZeroUsize>>>>,
    make: fn(&Scope) -> F,
}

impl<F: Fact> Producer<F> for Slow<F> {
    fn id(&self) -> ProducerId {
        ProducerId::from(self.id)
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![LangId::from("rust")])
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn cost(&self) -> CostClass {
        self.cost
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(&self, ctx: &ProduceCtx, scope: &Scope, _: &mut dyn EventSink) -> Result<F> {
        self.made.fetch_add(1, Ordering::SeqCst);
        self.jobs.lock().unwrap().push(ctx.focus.jobs);
        if !ctx.cancel.wait(self.wait) {
            return Err(Error::Cancelled);
        }
        Ok((self.make)(scope))
    }
}

/// A `Slow` producer before its fact type is chosen.
struct Shape {
    id: &'static str,
    cost: CostClass,
    wait: Duration,
    made: Arc<AtomicUsize>,
    jobs: Arc<Mutex<Vec<Option<NonZeroUsize>>>>,
}

impl Shape {
    /// The producer of a default `F`.
    fn defaulted<F: Fact + Default>(self) -> Slow<F> {
        Slow {
            id: self.id,
            cost: self.cost,
            wait: self.wait,
            made: self.made,
            jobs: self.jobs,
            make: |_| F::default(),
        }
    }
}

/// One symbol per file in scope.
fn symbols_of(scope: &Scope) -> Symbols {
    Symbols {
        symbols: scope
            .files
            .iter()
            .map(|file| SymbolDef {
                name: file.clone(),
                namespace: String::new(),
                display: file.clone(),
                kind: SymbolKind::Function,
                visibility: Visibility::Public,
                lang: LangId::from("rust"),
                file: file.clone(),
                span: Span {
                    line: 1,
                    col: 1,
                    end_line: 2,
                    end_col: 1,
                },
                body: None,
                parameters: 0,
            })
            .collect(),
    }
}

/// A check of `tier` and `cost` needing `needs`, planned over its symbols, that passes.
struct Graded(cupel_core::Manifest);

impl Check for Graded {
    fn manifest(&self) -> &cupel_core::Manifest {
        &self.0
    }

    fn plan(&self, _: &CheckCtx, scope: &Scope) -> Result<Plan> {
        let targets = scope.symbols.as_ref().map_or(0, |keys| keys.len() as u64);
        Ok(Plan::estimate(self.0.cost, targets))
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        _: &Facts,
        _: &Plan,
        _: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        ctx.cancel.checkpoint()?;
        Ok(CheckResult {
            id: self.0.id.clone(),
            status: CheckStatus::Pass,
            tier: self.0.tier,
            cost: self.0.cost,
            score: None,
            metrics: std::collections::BTreeMap::new(),
            delta: None,
            findings: 0,
            duration_ms: 0,
            skipped: None,
            oracle: None,
            notes: Vec::new(),
        })
    }
}

fn graded(id: &str, tier: u8, cost: CostClass, needs: &[FactKind]) -> Box<dyn Check> {
    Box::new(Graded(cupel_core::Manifest {
        tier: Tier::try_from(tier).unwrap(),
        cost,
        ..support::manifest(id, needs, &[])
    }))
}

/// Registries with `Symbols` (quick, instant), `Decisions` (quick, instant), `ModuleGraph`
/// (standard, instant), and `TestFrames` (standard, waiting `slow`), all for rust.
struct World {
    dir: tempfile::TempDir,
    checks: CheckRegistry,
    producers: ProducerRegistry,
    config: Config,
    graphs_made: Arc<AtomicUsize>,
    jobs_seen: Arc<Mutex<Vec<Option<NonZeroUsize>>>>,
}

impl World {
    fn new(checks: Vec<Box<dyn Check>>, slow: Duration) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let graphs_made = Arc::new(AtomicUsize::new(0));
        let jobs_seen = Arc::new(Mutex::new(Vec::new()));
        let none = Arc::new(AtomicUsize::new(0));
        let producer = |id, cost, wait, made: &Arc<AtomicUsize>| Shape {
            id,
            cost,
            wait,
            made: Arc::clone(made),
            jobs: Arc::clone(&jobs_seen),
        };
        let mut producers = ProducerRegistry::default();
        producers.register::<Symbols, _>(Slow {
            make: symbols_of,
            ..producer("lang.symbols", CostClass::Quick, Duration::ZERO, &none).defaulted()
        });
        let decisions = producer("lang.decisions", CostClass::Quick, Duration::ZERO, &none);
        producers.register::<Decisions, _>(decisions.defaulted());
        let graph = producer(
            "tool.graph",
            CostClass::Standard,
            Duration::ZERO,
            &graphs_made,
        );
        producers.register::<ModuleGraph, _>(graph.defaulted());
        let frames = producer("tool.frames", CostClass::Standard, slow, &none);
        producers.register::<TestFrames, _>(frames.defaulted());
        let mut registry = CheckRegistry::default();
        for check in checks {
            registry.register(check).unwrap();
        }
        let config = support::config_at(dir.path(), "");
        Self {
            dir,
            checks: registry,
            producers,
            config,
            graphs_made,
            jobs_seen,
        }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn execute(&self, resources: Resources, dry_run: bool) -> (Outcome, Vec<Event>) {
        let writer = Writer::new(self.root(), false).unwrap();
        let mut job = support::job(&writer, &self.config, &self.checks, &["rust"]);
        job.resources = resources;
        let scheduler = Scheduler {
            checks: &self.checks,
            producers: &self.producers,
        };
        let mut events = Vec::new();
        let outcome = if dry_run {
            scheduler.plan(&job, &mut events)
        } else {
            scheduler.run(&job, &mut events)
        };
        (outcome.unwrap(), events)
    }

    fn run(&self, resources: Resources) -> (Outcome, Vec<Event>) {
        self.execute(resources, false)
    }
}

/// A budget of `ms` milliseconds from now.
fn budget(ms: u64) -> Budget {
    let limit = Duration::from_millis(ms);
    Budget {
        limit,
        deadline: Instant::now() + limit,
    }
}

fn with_budget(ms: u64) -> Resources {
    Resources {
        budget: Some(budget(ms)),
        ..Resources::default()
    }
}

/// History in which `producer` for rust ran three times in `ms` milliseconds.
fn recorded(history: &mut History, producer: &str, ms: u64) {
    history.insert(
        TaskShape::producer(&ProducerId::from(producer), &LangId::from("rust"), false),
        vec![ms; 3],
    );
}

fn result<'a>(outcome: &'a Outcome, id: &str) -> &'a CheckResult {
    outcome
        .checks
        .iter()
        .find(|check| check.id.as_str() == id)
        .unwrap_or_else(|| panic!("no {id} in {:?}", outcome.checks))
}

/// The structure (cheap static check, costly check) both tests below use.
fn cheap_and_costly() -> Vec<Box<dyn Check>> {
    vec![
        graded(
            "static.cheap",
            0,
            CostClass::Quick,
            &[FactKind::Symbols, FactKind::Decisions],
        ),
        graded(
            "tests.costly",
            3,
            CostClass::Standard,
            &[FactKind::ModuleGraph],
        ),
    ]
}

#[test]
fn a_check_whose_closure_does_not_fit_is_skipped_for_budget_with_its_estimate() {
    let world = World::new(cheap_and_costly(), Duration::ZERO);
    let (outcome, events) = world.run(with_budget(5_000));
    let cheap = result(&outcome, "static.cheap");
    assert_eq!(cheap.status, CheckStatus::Pass);
    let costly = result(&outcome, "tests.costly");
    assert_eq!(costly.status, CheckStatus::Skipped);
    let skipped = costly.skipped.as_ref().unwrap();
    assert_eq!(skipped.reason, SkipReason::Budget);
    // The graph producer's standard default (30 s) plus the check's own 2 × 100 ms.
    assert_eq!(skipped.est_ms, Some(30_200));
    assert_eq!(
        world.graphs_made.load(Ordering::SeqCst),
        0,
        "its closure never ran"
    );
    assert!(outcome.interrupted, "a budget skip makes the run partial");
    assert!(events.contains(&Event::Skip {
        id: CheckId::from("tests.costly"),
        reason: SkipReason::Budget,
    }));
}

#[test]
fn without_a_budget_everything_runs_and_nothing_is_partial() {
    let world = World::new(cheap_and_costly(), Duration::ZERO);
    let (outcome, _) = world.run(Resources::default());
    assert!(
        outcome
            .checks
            .iter()
            .all(|check| check.status == CheckStatus::Pass)
    );
    assert!(!outcome.interrupted);
}

#[test]
fn recorded_durations_replace_the_default_and_let_the_check_fit() {
    let world = World::new(cheap_and_costly(), Duration::ZERO);
    let mut history = History::default();
    recorded(&mut history, "tool.graph", 40);
    let (outcome, _) = world.run(Resources {
        budget: Some(budget(5_000)),
        history,
        ..Resources::default()
    });
    assert_eq!(result(&outcome, "tests.costly").status, CheckStatus::Pass);
    assert_eq!(world.graphs_made.load(Ordering::SeqCst), 1);
    assert!(!outcome.interrupted);
}

#[test]
fn a_dry_run_plans_the_same_skips_and_honest_estimates() {
    let world = World::new(cheap_and_costly(), Duration::ZERO);
    let (outcome, _) = world.execute(with_budget(5_000), true);
    let plan = outcome.plan.unwrap();
    let skipped: Vec<_> = plan
        .would_skip
        .iter()
        .map(|skip| (skip.id.as_str(), skip.skipped.reason, skip.skipped.est_ms))
        .collect();
    assert_eq!(
        skipped,
        [("tests.costly", SkipReason::Budget, Some(30_200))]
    );
    let planned: Vec<_> = plan
        .would_run
        .iter()
        .map(|check| (check.id.as_str(), check.est_ms))
        .collect();
    // Symbols are made while planning; Decisions' quick default (100 ms) plus 2 × 1 ms.
    assert_eq!(planned, [("static.cheap", 102)]);
    assert_eq!(world.graphs_made.load(Ordering::SeqCst), 0);
}

#[test]
fn under_a_budget_the_most_valuable_check_runs_first_and_results_keep_registry_order() {
    let checks = vec![
        graded("static.low", 0, CostClass::Quick, &[FactKind::Decisions]),
        graded("tests.high", 3, CostClass::Quick, &[FactKind::Decisions]),
    ];
    let world = World::new(checks, Duration::ZERO);
    let (outcome, events) = world.run(with_budget(60_000));
    let started: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            Event::CheckStart { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(started, ["tests.high", "static.low"]);
    let ids: Vec<&str> = outcome
        .checks
        .iter()
        .map(|check| check.id.as_str())
        .collect();
    assert_eq!(ids, ["static.low", "tests.high"]);
}

#[test]
fn a_task_running_at_the_deadline_is_cut_and_the_run_ends_near_the_budget() {
    let checks = vec![graded(
        "tests.slow",
        2,
        CostClass::Standard,
        &[FactKind::TestFrames],
    )];
    let world = World::new(checks, Duration::from_secs(60));
    let mut history = History::default();
    // History says the frames take 10 ms, so the knapsack takes the check; they take a minute.
    recorded(&mut history, "tool.frames", 10);
    let started = Instant::now();
    let (outcome, _) = world.run(Resources {
        budget: Some(budget(300)),
        history,
        ..Resources::default()
    });
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_secs(5), "took {elapsed:?}");
    assert!(elapsed >= Duration::from_millis(250), "took {elapsed:?}");
    assert_eq!(result(&outcome, "tests.slow").status, CheckStatus::Partial);
    assert!(outcome.interrupted);
}

#[test]
fn a_task_the_knapsack_took_is_skipped_when_an_earlier_one_overran() {
    let checks = vec![
        graded("tests.first", 3, CostClass::Quick, &[FactKind::TestFrames]),
        graded(
            "tests.second",
            0,
            CostClass::Quick,
            &[FactKind::ModuleGraph],
        ),
    ];
    let world = World::new(checks, Duration::from_millis(600));
    let mut history = History::default();
    recorded(&mut history, "tool.frames", 10);
    recorded(&mut history, "tool.graph", 400);
    let (outcome, _) = world.run(Resources {
        budget: Some(budget(800)),
        history,
        ..Resources::default()
    });
    assert_eq!(result(&outcome, "tests.first").status, CheckStatus::Pass);
    let second = result(&outcome, "tests.second");
    assert_eq!(second.status, CheckStatus::Skipped);
    assert_eq!(second.skipped.as_ref().unwrap().reason, SkipReason::Budget);
    assert_eq!(world.graphs_made.load(Ordering::SeqCst), 0);
}

#[test]
fn every_production_and_check_run_is_timed_for_the_store() {
    let world = World::new(cheap_and_costly(), Duration::ZERO);
    let (outcome, _) = world.run(Resources::default());
    let rust = LangId::from("rust");
    let shapes: Vec<&TaskShape> = outcome.durations.iter().map(|timed| &timed.shape).collect();
    for expected in [
        TaskShape::producer(&ProducerId::from("lang.symbols"), &rust, false),
        TaskShape::producer(&ProducerId::from("lang.decisions"), &rust, false),
        TaskShape::producer(&ProducerId::from("tool.graph"), &rust, false),
        TaskShape::check(&CheckId::from("static.cheap"), &rust, 2),
        TaskShape::check(&CheckId::from("tests.costly"), &rust, 2),
    ] {
        assert!(shapes.contains(&&expected), "{expected:?} in {shapes:?}");
    }
    assert!(
        shapes
            .iter()
            .all(|shape| [TaskKind::Check, TaskKind::Producer].contains(&shape.kind))
    );
}

#[test]
fn the_concurrency_cap_reaches_producers() {
    let checks = vec![graded(
        "tests.costly",
        3,
        CostClass::Standard,
        &[FactKind::ModuleGraph],
    )];
    let world = World::new(checks, Duration::ZERO);
    world.run(Resources {
        jobs: NonZeroUsize::new(1),
        ..Resources::default()
    });
    let seen = world.jobs_seen.lock().unwrap();
    assert!(!seen.is_empty());
    assert!(
        seen.iter().all(|jobs| *jobs == NonZeroUsize::new(1)),
        "{seen:?}"
    );
}
