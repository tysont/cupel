// ABOUTME: A deferred producer, one the repository's own entry point overrules (ADR-162), is no candidate: not a
// ABOUTME: fallback after the detected ones, as an undetected producer is (a `[lang.<id>]` pin still chooses it).
mod support;

use cupel_core::syntax::Roots;
use cupel_core::{
    Availability, CostClass, Ctx, EventSink, FactKind, Langs, ProduceCtx, Producer, ProducerId,
    ProducerRegistry, Result, Scope,
};

/// A ready producer of `Roots` that is detected, deferred, or neither.
struct Candidate {
    id: &'static str,
    detected: bool,
    deferred: bool,
}

impl Producer<Roots> for Candidate {
    fn id(&self) -> ProducerId {
        ProducerId::from(self.id)
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

    fn detected(&self, _: &Ctx) -> bool {
        self.detected
    }

    fn deferred(&self, _: &Ctx) -> bool {
        self.deferred
    }

    fn produce(&self, _: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<Roots> {
        Ok(Roots::default())
    }
}

/// The candidates for `Roots` in rust, by ID, with nothing pinned.
fn candidates() -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    let mut producers = ProducerRegistry::default();
    producers.register::<Roots, _>(Candidate {
        id: "tool.deferred",
        detected: true,
        deferred: true,
    });
    producers.register::<Roots, _>(Candidate {
        id: "tool.fallback",
        detected: false,
        deferred: false,
    });
    let ctx = Ctx::new(dir.path().to_path_buf());
    producers
        .candidates(FactKind::Roots, &support::lang("rust", "rust"), &ctx)
        .unwrap()
        .iter()
        .map(|producer| producer.id().to_string())
        .collect()
}

#[test]
fn a_deferred_producer_is_no_candidate_and_an_undetected_one_stays_a_fallback() {
    assert_eq!(candidates(), ["tool.fallback"]);
}
