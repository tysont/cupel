// ABOUTME: The run's seed reaches producers (ADR-078): what the scheduler hands every producer is the job's
// ABOUTME: seed, so an engine that varies its work by seed repeats it under `--seed`.
mod support;

use std::sync::{Arc, Mutex};

use cupel_core::syntax::Symbols;
use cupel_core::{
    Availability, CheckRegistry, CostClass, Ctx, EventSink, FactKind, Langs, ProduceCtx, Producer,
    ProducerId, ProducerRegistry, Result, Scheduler, Scope, Writer,
};

/// A `Symbols` producer for every language that records the seed it was given.
struct Seen(Arc<Mutex<Vec<u64>>>);

impl Producer<Symbols> for Seen {
    fn id(&self) -> ProducerId {
        ProducerId::from("lang.seen")
    }

    fn languages(&self) -> Langs {
        Langs::Any
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn cost(&self) -> CostClass {
        CostClass::Quick
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<Symbols> {
        self.0.lock().unwrap().push(ctx.seed);
        Ok(Symbols::default())
    }
}

#[test]
fn producers_receive_the_jobs_seed() {
    let dir = tempfile::tempdir().unwrap();
    let config = support::config_at(dir.path(), "");
    let writer = Writer::new(dir.path(), false).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut producers = ProducerRegistry::default();
    producers.register::<Symbols, _>(Seen(Arc::clone(&seen)));
    let checks = CheckRegistry::default();
    let mut job = support::job(&writer, &config, &checks, &["rust"]);
    job.seed = 4242;

    Scheduler {
        checks: &checks,
        producers: &producers,
    }
    .run(&job, &mut Vec::new())
    .unwrap();

    assert_eq!(*seen.lock().unwrap(), [4242]);
}

#[test]
fn a_produce_context_built_outside_the_scheduler_has_seed_zero() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ProduceCtx::new(
        Ctx::new(dir.path().to_path_buf()),
        Writer::new(dir.path(), false).unwrap(),
        cupel_core::Cancellation::new(),
    );
    assert_eq!(ctx.seed, 0);
}
