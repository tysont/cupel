// ABOUTME: How tool producers join the producer registry (ADR-074): a producer submitted from any
// ABOUTME: module of the linked binary, here this test, is registered by `cupel_adapt::register`.
use cupel_adapt::ToolProducer;
use cupel_core::syntax::Symbols;
use cupel_core::{
    Availability, CostClass, Ctx, EventSink, FactKind, Lang, LangFamily, LangId, Langs, ProduceCtx,
    Producer, ProducerId, ProducerRegistry, Result, Scope,
};

/// A producer that exists only to be registered.
struct Probe;

impl Producer<Symbols> for Probe {
    fn id(&self) -> ProducerId {
        ProducerId::from("tool.probe")
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![LangId::from("rust")])
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

    fn produce(&self, _: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<Symbols> {
        unreachable!("registration never produces")
    }
}

inventory::submit!(ToolProducer {
    id: "tool.probe",
    register: |registry| registry.register::<Symbols, _>(Probe),
});

fn ids(registry: &ProducerRegistry, lang: &str) -> Vec<String> {
    let lang = Lang {
        id: LangId::from(lang),
        family: LangFamily::from(lang),
    };
    registry
        .producers_for(FactKind::Symbols, &lang)
        .map(|producer| producer.id().to_string())
        .collect()
}

#[test]
fn a_submitted_tool_producer_is_registered_for_its_languages() {
    let mut registry = ProducerRegistry::default();

    cupel_adapt::register(&mut registry);

    assert_eq!(ids(&registry, "rust"), ["tool.probe"]);
    assert_eq!(ids(&registry, "go"), Vec::<String>::new());
}
