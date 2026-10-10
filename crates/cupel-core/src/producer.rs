// ABOUTME: The producer contract and registry: anything that makes a fact, for some languages.
// ABOUTME: The registry maps (fact kind, language) to producers in selection order (design §8.3.3).
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::path::PathBuf;

use serde_json::{Map, Value};

use crate::error::Result;
use crate::event::EventSink;
use crate::fact::{Fact, FactKind};
use crate::grade::CostClass;
use crate::id::{ProducerId, ToolId};
use crate::lang::{Lang, Langs};
use crate::scope::Scope;

/// What a producer can see when reporting whether it can run here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ctx {
    /// The repository root (`--cwd`).
    pub root: PathBuf,
}

/// What a producer can see while producing. Grows cancellation as the run infrastructure
/// lands (P0.7).
#[derive(Debug, Clone, PartialEq)]
pub struct ProduceCtx {
    pub ctx: Ctx,
    /// The `[lang.<lang>]` table for the language being produced (`Config::lang_config`).
    /// Producers never see `[checks.*]` (design §6.7).
    pub config: Map<String, Value>,
}

/// Whether a producer can run in this environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Ready,
    /// A tool is not installed; `install` is a command or hint that installs it.
    MissingTool {
        name: ToolId,
        install: Option<String>,
    },
    /// A configuration key must be set, such as `lang.rust.test`.
    Unconfigured {
        key: String,
    },
}

/// Makes one kind of fact. Language producers, tool producers, and engines all implement this.
pub trait Producer<F: Fact>: Send + Sync {
    /// Such as `lang.go`, `tool.pytest-cov`, or `engine.mutation`.
    fn id(&self) -> ProducerId;
    fn languages(&self) -> Langs;
    /// Facts this producer consumes.
    fn needs(&self) -> &[FactKind];
    fn cost(&self) -> CostClass;
    fn availability(&self, ctx: &Ctx) -> Availability;
    fn produce(&self, ctx: &ProduceCtx, scope: &Scope, sink: &mut dyn EventSink) -> Result<F>;
}

/// A producer as the registry holds it, with the fact type erased.
pub trait RegisteredProducer: Send + Sync {
    fn provides(&self) -> FactKind;
    fn id(&self) -> ProducerId;
    fn languages(&self) -> Langs;
    fn needs(&self) -> &[FactKind];
    fn cost(&self) -> CostClass;
    fn availability(&self, ctx: &Ctx) -> Availability;
}

/// Binds a producer to the fact type it makes.
struct Registration<F, P> {
    producer: P,
    fact: PhantomData<fn() -> F>,
}

impl<F: Fact, P: Producer<F>> RegisteredProducer for Registration<F, P> {
    fn provides(&self) -> FactKind {
        F::KIND
    }

    fn id(&self) -> ProducerId {
        self.producer.id()
    }

    fn languages(&self) -> Langs {
        self.producer.languages()
    }

    fn needs(&self) -> &[FactKind] {
        self.producer.needs()
    }

    fn cost(&self) -> CostClass {
        self.producer.cost()
    }

    fn availability(&self, ctx: &Ctx) -> Availability {
        self.producer.availability(ctx)
    }
}

/// Every registered producer, by the fact kind it provides, in registration order.
#[derive(Default)]
pub struct ProducerRegistry {
    by_kind: BTreeMap<FactKind, Vec<Box<dyn RegisteredProducer>>>,
}

impl ProducerRegistry {
    pub fn register<F: Fact, P: Producer<F> + 'static>(&mut self, producer: P) {
        self.by_kind
            .entry(F::KIND)
            .or_default()
            .push(Box::new(Registration {
                producer,
                fact: PhantomData,
            }));
    }

    /// Producers of `kind` that serve `lang`, in selection order.
    pub fn producers_for<'a>(
        &'a self,
        kind: FactKind,
        lang: &Lang,
    ) -> impl Iterator<Item = &'a dyn RegisteredProducer> + use<'a> {
        let lang = lang.clone();
        self.by_kind
            .get(&kind)
            .into_iter()
            .flatten()
            .map(AsRef::as_ref)
            .filter(move |producer| producer.languages().covers(&lang))
    }
}
