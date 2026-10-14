// ABOUTME: The producer contract and registry: anything that makes a fact, for some languages.
// ABOUTME: The registry maps (fact kind, language) to producers in selection order (design §8.3.3).
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Map, Value};

use crate::cancel::Cancellation;
use crate::config::Table;
use crate::error::Result;
use crate::event::EventSink;
use crate::exclude::Exclude;
use crate::fact::{Fact, FactKind};
use crate::grade::CostClass;
use crate::id::{LangId, ProducerId, ToolId};
use crate::lang::{Lang, Langs};
use crate::runner::Runner;
use crate::scope::Scope;
use crate::test_results::TestResults;
use crate::writer::Writer;

/// What a producer can see when reporting whether it can run here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ctx {
    /// The repository root (`--cwd`).
    pub root: PathBuf,
    /// Paths the run never considers (`[scope] exclude`, ADR-065); producers that read the
    /// repository rather than the scope (project roots) must not descend into them.
    pub exclude: Exclude,
    /// The `[lang.<id>]` tables, which pin producers (ADR-076); empty when nothing is configured.
    pub lang: BTreeMap<LangId, Table>,
}

impl Ctx {
    /// The context of the repository at `root`, excluding only what every run excludes.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            exclude: Exclude::default(),
            lang: BTreeMap::new(),
        }
    }

    /// The producer name `[lang.<lang>]` pins for `kind`, such as `unittest`.
    pub fn pin(&self, kind: FactKind, lang: &LangId) -> Option<&str> {
        self.lang.get(lang)?.get(kind.pin_key()?)?.as_str()
    }
}

/// What a producer can see while producing.
#[derive(Debug, Clone)]
pub struct ProduceCtx {
    pub ctx: Ctx,
    /// The `[lang.<lang>]` table for the language being produced (`Config::lang_config`).
    /// Producers never see `[checks.*]` (design §6.7).
    pub config: Map<String, Value>,
    /// Writes only under `.cupel/` (`Writer::confined`), such as `.cupel/cache/`.
    pub writer: Writer,
    /// Fires on `--timeout`; poll it between units of work.
    pub cancel: Cancellation,
    /// The facts the producer's `needs` name, produced first for the same language.
    pub upstream: Upstream,
}

impl ProduceCtx {
    /// The context of producing in `ctx`'s repository with `writer` and `cancel`, with no
    /// `[lang.*]` configuration and no upstream facts.
    pub fn new(ctx: Ctx, writer: Writer, cancel: Cancellation) -> Self {
        Self {
            ctx,
            config: Map::new(),
            writer,
            cancel,
            upstream: Upstream::default(),
        }
    }
}

/// The facts a producer needs, as the scheduler produced them for it.
#[derive(Debug, Clone, Default)]
pub struct Upstream(BTreeMap<FactKind, SharedFact>);

impl Upstream {
    pub fn insert(&mut self, fact: SharedFact) {
        self.0.insert(fact.kind(), fact);
    }

    /// The fact of type `F`, when it was produced.
    pub fn get<F: Fact>(&self) -> Option<&F> {
        self.0.get(&F::KIND)?.downcast_ref()
    }
}

/// The program a tool producer runs, and the command that prints its version (for `doctor`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolProbe {
    pub name: ToolId,
    pub version: Vec<String>,
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
    /// Whether the repository shows signs of using this producer (a tool producer's
    /// toolchain-table detection rules). Detected producers are preferred over the rest (ADR-076).
    fn detected(&self, _ctx: &Ctx) -> bool {
        true
    }
    /// The program this producer runs, for producers that run one.
    fn tool(&self, _ctx: &Ctx) -> Option<ToolProbe> {
        None
    }
    fn produce(&self, ctx: &ProduceCtx, scope: &Scope, sink: &mut dyn EventSink) -> Result<F>;
}

/// A fact a producer made, with its type erased: what [`RegisteredProducer::produce`] returns.
/// Callers that know the kind take the typed value back with [`downcast`](Self::downcast);
/// callers that do not (the conformance harness, `explain`) read it as JSON.
pub struct Produced {
    kind: FactKind,
    value: Box<dyn Any + Send + Sync>,
    to_json: fn(&(dyn Any + Send + Sync)) -> Value,
}

impl Produced {
    pub fn new<F: Fact>(fact: F) -> Self {
        Self {
            kind: F::KIND,
            value: Box::new(fact),
            to_json: |value| {
                value
                    .downcast_ref::<F>()
                    .and_then(|fact| serde_json::to_value(fact).ok())
                    .unwrap_or(Value::Null)
            },
        }
    }

    pub fn kind(&self) -> FactKind {
        self.kind
    }

    pub fn downcast_ref<F: Fact>(&self) -> Option<&F> {
        self.value.downcast_ref()
    }

    pub fn downcast<F: Fact>(self) -> Option<F> {
        self.value.downcast().ok().map(|fact| *fact)
    }

    /// The fact as it serializes.
    pub fn to_json(&self) -> Value {
        (self.to_json)(self.value.as_ref())
    }

    /// This fact behind an `Arc`, so several checks can read one production.
    pub fn share(self) -> SharedFact {
        SharedFact {
            kind: self.kind,
            value: Arc::from(self.value),
            to_json: self.to_json,
        }
    }
}

/// A produced fact several checks share; what [`Facts::insert_shared`](crate::Facts) takes.
#[derive(Clone)]
pub struct SharedFact {
    kind: FactKind,
    value: Arc<dyn Any + Send + Sync>,
    to_json: fn(&(dyn Any + Send + Sync)) -> Value,
}

impl SharedFact {
    pub fn kind(&self) -> FactKind {
        self.kind
    }

    pub fn downcast_ref<F: Fact>(&self) -> Option<&F> {
        self.value.downcast_ref()
    }

    /// The fact as it serializes.
    pub fn to_json(&self) -> Value {
        (self.to_json)(self.value.as_ref())
    }

    pub(crate) fn value(&self) -> Arc<dyn Any + Send + Sync> {
        Arc::clone(&self.value)
    }
}

impl fmt::Debug for SharedFact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedFact")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for Produced {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Produced")
            .field("kind", &self.kind)
            .field("value", &(self.to_json)(self.value.as_ref()))
            .finish()
    }
}

/// A producer as the registry holds it, with the fact type erased.
pub trait RegisteredProducer: Send + Sync {
    fn provides(&self) -> FactKind;
    fn id(&self) -> ProducerId;
    fn languages(&self) -> Langs;
    fn needs(&self) -> &[FactKind];
    fn cost(&self) -> CostClass;
    fn availability(&self, ctx: &Ctx) -> Availability;
    fn detected(&self, ctx: &Ctx) -> bool;
    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe>;
    /// Runs the producer and returns its fact with the type erased.
    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        sink: &mut dyn EventSink,
    ) -> Result<Produced>;
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

    fn detected(&self, ctx: &Ctx) -> bool {
        self.producer.detected(ctx)
    }

    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe> {
        self.producer.tool(ctx)
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        sink: &mut dyn EventSink,
    ) -> Result<Produced> {
        self.producer.produce(ctx, scope, sink).map(Produced::new)
    }
}

/// Every registered producer, by the fact kind it provides, in registration order.
#[derive(Default)]
pub struct ProducerRegistry {
    by_kind: BTreeMap<FactKind, Vec<Box<dyn RegisteredProducer>>>,
    /// The `TestResults` producers registered as runners, by ID (ADR-077).
    runners: BTreeMap<ProducerId, Box<dyn Runner>>,
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

    /// Registers `runner` as a `TestResults` producer and keeps it for [`runner`](Self::runner).
    pub fn register_runner<R: Runner + Clone + 'static>(&mut self, runner: R) {
        self.runners.insert(runner.id(), Box::new(runner.clone()));
        self.register::<TestResults, _>(runner);
    }

    /// The runner registered as `id`.
    pub fn runner(&self, id: &ProducerId) -> Option<&dyn Runner> {
        self.runners.get(id).map(AsRef::as_ref)
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

    /// The producers of `kind` for `lang` a run may use, in preference order (ADR-076): only
    /// the producer `[lang.<lang>]` pins, when it pins one; otherwise the detected producers in
    /// registration order, then the rest. `Err` holds the pin's configuration key when the pin
    /// names no producer of `kind` for `lang`.
    pub fn candidates<'a>(
        &'a self,
        kind: FactKind,
        lang: &Lang,
        ctx: &Ctx,
    ) -> std::result::Result<Vec<&'a dyn RegisteredProducer>, String> {
        let all = self.producers_for(kind, lang);
        if let Some(pin) = ctx.pin(kind, &lang.id) {
            let pinned: Vec<_> = all
                .filter(|producer| {
                    let id = producer.id();
                    id.as_str() == pin || id.as_str().strip_prefix("tool.") == Some(pin)
                })
                .collect();
            return if pinned.is_empty() {
                Err(format!(
                    "lang.{}.{}",
                    lang.id,
                    kind.pin_key().unwrap_or_default()
                ))
            } else {
                Ok(pinned)
            };
        }
        let (detected, rest): (Vec<_>, Vec<_>) = all.partition(|producer| producer.detected(ctx));
        Ok(detected.into_iter().chain(rest).collect())
    }
}
