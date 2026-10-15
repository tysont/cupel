// ABOUTME: What the scheduler hands a producer beside its needs (ADR-080): the facts it wants when they can be
// ABOUTME: made, the registered runners, the run's depth, and the lines a range changed.
mod support;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use cupel_core::coverage::CoverageMap;
use cupel_core::syntax::{ProjectInfo, Symbols};
use cupel_core::{
    Availability, CheckRegistry, CostClass, Ctx, Error, EventSink, FactKind, Focus, Langs,
    ProduceCtx, Producer, ProducerId, ProducerRegistry, Result, Scheduler, Scope, Writer,
};

/// What the recording producer saw: the kinds upstream held and the runners it was handed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Seen {
    project_info: bool,
    coverage: bool,
    runners: String,
    focus: Option<Focus>,
}

/// A `Symbols` producer that wants `ProjectInfo` and `CoverageMap` and records what it got.
struct Recording(Arc<Mutex<Option<Seen>>>);

impl Producer<Symbols> for Recording {
    fn id(&self) -> ProducerId {
        ProducerId::from("lang.recording")
    }

    fn languages(&self) -> Langs {
        Langs::Any
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn wants(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo, FactKind::CoverageMap]
    }

    fn cost(&self) -> CostClass {
        CostClass::Quick
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<Symbols> {
        *self.0.lock().unwrap() = Some(Seen {
            project_info: ctx.upstream.get::<ProjectInfo>().is_some(),
            coverage: ctx.upstream.get::<CoverageMap>().is_some(),
            runners: format!("{:?}", ctx.runners),
            focus: Some(ctx.focus.clone()),
        });
        Ok(Symbols::default())
    }
}

/// Project roots: none, made without fault.
struct Roots;

impl Producer<ProjectInfo> for Roots {
    fn id(&self) -> ProducerId {
        ProducerId::from("lang.roots")
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

    fn produce(&self, _: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<ProjectInfo> {
        Ok(ProjectInfo::default())
    }
}

/// A coverage producer that always fails.
struct Broken;

impl Producer<CoverageMap> for Broken {
    fn id(&self) -> ProducerId {
        ProducerId::from("tool.broken")
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

    fn produce(&self, _: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<CoverageMap> {
        Err(Error::InvalidValue {
            what: "coverage",
            value: "broken".to_owned(),
            expected: "nothing; this producer always fails",
        })
    }
}

/// The lines the range of [`seen`]'s job changed.
fn changed() -> BTreeMap<String, Vec<(u32, u32)>> {
    BTreeMap::from([("a.x".to_owned(), vec![(3, 4)])])
}

/// Runs a deep job over rust, with a range that changed lines 3–4 of `a.x`, that produces
/// `Symbols` with the recording producer, the roots, and the broken coverage producer
/// registered; what the recording producer saw.
fn seen() -> Seen {
    let dir = tempfile::tempdir().unwrap();
    let config = support::config_at(dir.path(), "");
    let writer = Writer::new(dir.path(), false).unwrap();
    let recorded = Arc::new(Mutex::new(None));
    let mut producers = ProducerRegistry::default();
    producers.register::<Symbols, _>(Recording(Arc::clone(&recorded)));
    producers.register::<ProjectInfo, _>(Roots);
    producers.register::<CoverageMap, _>(Broken);
    let checks = CheckRegistry::default();
    let mut job = support::job(&writer, &config, &checks, &["rust"]);
    job.depth = CostClass::Deep;
    job.changed = Some(changed());
    Scheduler {
        checks: &checks,
        producers: &producers,
    }
    .run(&job, &mut Vec::new())
    .unwrap();
    let seen = recorded.lock().unwrap().clone();
    seen.expect("the scheduler produced symbols")
}

#[test]
fn a_producer_gets_the_facts_it_wants_that_can_be_made_and_runs_without_the_rest() {
    let seen = seen();
    assert_eq!(
        (seen.project_info, seen.coverage),
        (true, false),
        "{seen:?}"
    );
}

#[test]
fn a_producer_gets_the_registered_runners() {
    assert_eq!(seen().runners, "[]");
}

#[test]
fn a_producer_gets_the_runs_depth_and_changed_lines() {
    assert_eq!(
        seen().focus,
        Some(Focus {
            depth: CostClass::Deep,
            symbols: None,
            changed: Some(changed()),
        })
    );
}
