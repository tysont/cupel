// ABOUTME: Tests for the `Runner` surface (ADR-077 §3): runners registered through the registry are found by
// ABOUTME: ID and as `TestResults` producers, and a per-call timeout ends only its own work.
use std::time::Duration;

use cupel_core::{
    Availability, Cancellation, CostClass, Ctx, EventSink, FactKind, Langs, ProduceCtx, Producer,
    ProducerId, ProducerRegistry, Result, Runner, RunnerCommand, Scope, TestResults, TestSelection,
};

mod support;

/// A runner that runs nothing: it lists two tests and reports none.
#[derive(Clone)]
struct Listing;

impl Producer<TestResults> for Listing {
    fn id(&self) -> ProducerId {
        ProducerId::from("tool.listing")
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

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<TestResults> {
        self.run(ctx, &TestSelection::All, &[], None)
    }
}

impl Runner for Listing {
    fn list(&self, _: &ProduceCtx) -> Result<Vec<String>> {
        Ok(vec!["a".to_owned(), "b".to_owned()])
    }

    fn run(
        &self,
        ctx: &ProduceCtx,
        _: &TestSelection,
        _: &[(String, String)],
        _: Option<Duration>,
    ) -> Result<TestResults> {
        Ok(TestResults::new(
            self.id(),
            Vec::new(),
            Vec::new(),
            &ctx.ctx.root,
        ))
    }

    fn commands(&self, _: &ProduceCtx, selection: &TestSelection) -> Option<Vec<RunnerCommand>> {
        let TestSelection::Tests(ids) = selection else {
            return None;
        };
        Some(vec![RunnerCommand {
            dir: ".".to_owned(),
            command: format!("listing {}", ids.join(" ")),
        }])
    }
}

#[test]
fn a_registered_runner_is_found_by_id_and_produces_test_results() {
    let mut registry = ProducerRegistry::default();
    registry.register_runner(Listing);
    let lang = support::lang("python", "python");
    let producers: Vec<String> = registry
        .producers_for(FactKind::TestResults, &lang)
        .map(|producer| producer.id().to_string())
        .collect();
    assert_eq!(producers, ["tool.listing"]);
    let runner = registry.runner(&ProducerId::from("tool.listing")).unwrap();
    let ctx = ProduceCtx::new(
        Ctx::new(std::env::temp_dir()),
        cupel_core::Writer::new(&std::env::temp_dir(), false)
            .unwrap()
            .recording(),
        Cancellation::new(),
    );
    assert_eq!(runner.list(&ctx).unwrap(), ["a", "b"]);
    let commands = runner
        .commands(&ctx, &TestSelection::Tests(vec!["a".to_owned()]))
        .unwrap();
    assert_eq!(commands[0].command, "listing a");
    assert!(runner.commands(&ctx, &TestSelection::All).is_none());
    assert!(registry.runner(&ProducerId::from("tool.other")).is_none());
}

#[test]
fn a_child_cancellation_ends_at_its_timeout_without_cancelling_its_parent() {
    let parent = Cancellation::new();
    let child = parent.child(Duration::from_millis(20));
    assert!(!child.is_cancelled());
    assert!(!child.wait(Duration::from_secs(5)));
    assert!(child.is_cancelled());
    assert!(!parent.is_cancelled());
}

#[test]
fn a_child_cancellation_ends_with_its_parent() {
    let parent = Cancellation::new();
    let child = parent.child(Duration::from_secs(3600));
    parent.cancel();
    assert!(child.is_cancelled());
}
