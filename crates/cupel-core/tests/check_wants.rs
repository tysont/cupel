// ABOUTME: A check's wants are made only within the run's depth, as a producer's are (ADR-080, ADR-162): a quick run
// ABOUTME: never runs a costlier producer for an optional fact, so it executes nothing a `standard` producer would.
mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cupel_core::syntax::Roots;
use cupel_core::{
    Availability, CheckRegistry, CostClass, Ctx, EventSink, FactKind, Langs, ProduceCtx, Producer,
    ProducerId, ProducerRegistry, Result, Scheduler, Scope, Writer,
};

/// Entry points at `standard` cost, standing in for a fact made by running the project (a coverage
/// run); it records whether it ran.
struct Executing(Arc<AtomicBool>);

impl Producer<Roots> for Executing {
    fn id(&self) -> ProducerId {
        ProducerId::from("tool.executing")
    }

    fn languages(&self) -> Langs {
        Langs::Any
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn cost(&self) -> CostClass {
        CostClass::Standard
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(&self, _: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<Roots> {
        self.0.store(true, Ordering::SeqCst);
        Ok(Roots::default())
    }
}

/// Whether the `standard` producer ran in a run at `depth` of a `quick` check that wants its fact.
fn executed_at(depth: CostClass) -> bool {
    let dir = tempfile::tempdir().unwrap();
    let config = support::config_at(dir.path(), "");
    let writer = Writer::new(dir.path(), false).unwrap();
    let ran = Arc::new(AtomicBool::new(false));
    let mut producers = ProducerRegistry::default();
    producers.register::<Roots, _>(Executing(Arc::clone(&ran)));
    let mut checks = CheckRegistry::default();
    let manifest = support::manifest("wants.roots", &[], &[FactKind::Roots]);
    checks
        .register(Box::new(support::Passing(manifest)))
        .unwrap();
    let mut job = support::job(&writer, &config, &checks, &["rust"]);
    job.depth = depth;
    Scheduler {
        checks: &checks,
        producers: &producers,
    }
    .run(&job, &mut Vec::new())
    .unwrap();
    ran.load(Ordering::SeqCst)
}

#[test]
fn a_checks_want_is_made_only_by_a_run_deep_enough_for_its_producer() {
    assert_eq!(
        [
            executed_at(CostClass::Quick),
            executed_at(CostClass::Standard)
        ],
        [false, true]
    );
}

/// The facts a dry run at `depth` plans for the `quick` check that wants the entry points.
fn planned_at(depth: CostClass) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    let config = support::config_at(dir.path(), "");
    let writer = Writer::new(dir.path(), false).unwrap();
    let mut producers = ProducerRegistry::default();
    producers.register::<Roots, _>(Executing(Arc::new(AtomicBool::new(false))));
    let mut checks = CheckRegistry::default();
    let manifest = support::manifest("wants.roots", &[], &[FactKind::Roots]);
    checks
        .register(Box::new(support::Passing(manifest)))
        .unwrap();
    let mut job = support::job(&writer, &config, &checks, &["rust"]);
    job.depth = depth;
    let scheduler = Scheduler {
        checks: &checks,
        producers: &producers,
    };
    let outcome = scheduler.plan(&job, &mut Vec::new()).unwrap();
    outcome.plan.unwrap().would_run[0].needs.clone()
}

#[test]
fn a_dry_run_names_a_checks_want_it_leaves_out_with_its_cost() {
    assert_eq!(
        [
            planned_at(CostClass::Quick),
            planned_at(CostClass::Standard)
        ],
        [vec!["roots(standard)"], vec!["roots"]]
    );
}
