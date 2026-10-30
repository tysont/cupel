// ABOUTME: The producer contract and registry: anything that makes a fact, and runners, which also select tests.
// ABOUTME: The registry maps (fact kind, language) to producers in selection order (design §8.3.3, ADR-077).
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Value};

use crate::cancel::Cancellation;
use crate::config::Table;
use crate::error::Result;
use crate::event::EventSink;
use crate::exclude::Exclude;
use crate::fact::{Fact, FactKind, Produced, SharedFact};
use crate::flake::RunOrder;
use crate::grade::CostClass;
use crate::id::{LangId, ProducerId, ToolId};
use crate::lang::{Lang, Langs};
use crate::oracle::RunOracle;
use crate::scope::{ChangedLines, Scope};
use crate::syntax::SymbolKey;
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
    /// Producers never see a check's table (design §6.7).
    pub config: Map<String, Value>,
    /// The settings registered checks declare for the fact being produced
    /// (`SettingSpec::configures`), as `[checks."<id>"]` and its override for the language
    /// configure them; an unconfigured one is absent, and the producer applies its own default
    /// (ADR-147).
    pub settings: Map<String, Value>,
    /// Writes only under `.cupel/` (`Writer::confined`), such as `.cupel/cache/`.
    pub writer: Writer,
    /// Fires on `--timeout`; poll it between units of work.
    pub cancel: Cancellation,
    /// The facts the producer's `needs` name, produced first for the same language.
    pub upstream: Upstream,
    /// The run's seed (`--seed`, else random), for producers that vary their work by seed
    /// (ADR-078); 0 outside a scheduled run.
    pub seed: u64,
    /// The registered runners, for engines that run tests (ADR-080); none outside a scheduled
    /// run unless a caller adds them.
    pub runners: Runners,
    /// What the run asked to look at, for producers that narrow their work to it (ADR-080).
    pub focus: Focus,
    /// The oracle the run compares against (`--oracle`, ADR-091 §2), for producers that run it
    /// (ADR-093 §2); `None` outside a run whose checks use one.
    pub oracle: Option<RunOracle>,
}

/// What a run asked to look at beyond its files: its depth, the symbols `--symbols` selected,
/// the lines a range changed and the commit it starts from, how much it may run at once, and
/// its case budget. Producers that narrow their work
/// by it (the mutation engine, ADR-080) read it; the rest produce over their scope's files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Focus {
    pub depth: CostClass,
    /// The language's symbols `--symbols` selected; `None` for every symbol in scope.
    pub symbols: Option<BTreeSet<SymbolKey>>,
    /// Per file, the 1-based inclusive line ranges a range scope changed (`git diff -U0`);
    /// `None` without a range.
    pub changed: Option<ChangedLines>,
    /// The commit a range starts from (`--changed`'s merge base or `--base`), for producers
    /// that compare what they measure with it (ADR-103 §5); `None` without a range.
    pub base: Option<String>,
    /// How many processes or threads a producer may run at once (`--jobs`, ADR-086 §11);
    /// `None` for its own default.
    pub jobs: Option<NonZeroUsize>,
    /// `--cases`: how many inputs a generative producer generates per entry or relation, or the
    /// cases a property runs (ADR-094 §5, ADR-095 §3, ADR-098); `None` for the producer's own
    /// default, or [`case_budget`](Self::case_budget)'s for the depth.
    pub cases: Option<u64>,
}

impl Default for Focus {
    /// The default depth, every symbol, no range.
    fn default() -> Self {
        Self {
            depth: CostClass::Standard,
            symbols: None,
            changed: None,
            base: None,
            jobs: None,
            cases: None,
        }
    }
}

impl Focus {
    /// How many workers a producer that parallelizes its own work may run: `--jobs`, else the
    /// cores.
    pub fn workers(&self) -> usize {
        self.jobs
            .or_else(|| std::thread::available_parallelism().ok())
            .map_or(1, NonZeroUsize::get)
    }

    /// The generated cases a generative producer runs per property: `--cases`, else ten times
    /// as many at each depth above `quick`'s 100 (ADR-095 §3).
    pub fn case_budget(&self) -> u32 {
        let default = match self.depth {
            CostClass::Quick => 100,
            CostClass::Standard => 1_000,
            CostClass::Deep => 10_000,
            CostClass::Exhaustive => 100_000,
        };
        self.cases
            .map_or(default, |cases| u32::try_from(cases).unwrap_or(u32::MAX))
    }
}

impl ProduceCtx {
    /// The context of producing in `ctx`'s repository with `writer` and `cancel`, with no
    /// `[lang.*]` configuration, no settings, no upstream facts, and seed 0.
    pub fn new(ctx: Ctx, writer: Writer, cancel: Cancellation) -> Self {
        Self {
            ctx,
            config: Map::new(),
            settings: Map::new(),
            writer,
            cancel,
            upstream: Upstream::default(),
            seed: 0,
            runners: Runners::default(),
            focus: Focus::default(),
            oracle: None,
        }
    }
}

/// The runners a registry holds, by ID, shared with the producers it serves.
#[derive(Clone, Default)]
pub struct Runners(Arc<BTreeMap<ProducerId, Arc<dyn Runner>>>);

impl Runners {
    /// The runner registered as `id`.
    pub fn get(&self, id: &ProducerId) -> Option<&dyn Runner> {
        self.0.get(id).map(AsRef::as_ref)
    }
}

impl fmt::Debug for Runners {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.0.keys()).finish()
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

    /// The fact of `kind` as it serializes, when it was produced (a producer plugin's input,
    /// ADR-089 §3).
    pub fn json(&self, kind: FactKind) -> Option<Value> {
        self.0.get(&kind).map(SharedFact::to_json)
    }

    /// The kinds produced, in catalog order.
    pub fn kinds(&self) -> impl Iterator<Item = FactKind> + '_ {
        self.0.keys().copied()
    }
}

/// The program a tool producer runs, and the command that prints its version (for `doctor`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolProbe {
    pub name: ToolId,
    pub version: Vec<String>,
}

/// How a missing tool gets installed (ADR-081).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Install {
    /// A shell command that installs the tool when run in the repository root.
    Command(String),
    /// What to do by hand, for a program Cupel does not install, such as a language toolchain.
    Hint(String),
}

/// Whether a producer can run in this environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Ready,
    /// A tool is not installed; `install` says how to install it, when that is known.
    MissingTool {
        name: ToolId,
        install: Option<Install>,
    },
    /// A configuration key must be set, such as `lang.rust.test`.
    Unconfigured {
        key: String,
    },
}

/// What producing a fact here would cost the run now (ADR-083): the producer's cost class,
/// unless it can tell it would do less, such as refreshing a cache it keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estimate {
    pub cost: CostClass,
    /// How the producer would make the fact, such as `cached` or `refresh 3 tests`, for a dry
    /// run's `needs`; `None` when it would simply produce it.
    pub note: Option<String>,
}

impl Estimate {
    /// Producing at `cost`, with nothing to say about it.
    pub fn of(cost: CostClass) -> Self {
        Self { cost, note: None }
    }
}

/// Makes one kind of fact. Language producers, tool producers, and engines all implement this.
pub trait Producer<F: Fact>: Send + Sync {
    /// Such as `lang.go`, `tool.pytest-cov`, or `engine.mutation`.
    fn id(&self) -> ProducerId;
    fn languages(&self) -> Langs;
    /// Facts this producer consumes.
    fn needs(&self) -> &[FactKind];
    /// Facts this producer uses when they can be made, and does without otherwise (ADR-080).
    fn wants(&self) -> &[FactKind] {
        &[]
    }
    fn cost(&self) -> CostClass;
    /// What producing here would cost now; `cost()` unless the producer keeps a cache that
    /// makes it cheaper (ADR-083). The scheduler makes a want only when this is within the
    /// run's depth. `upstream` holds those of its needs and wants already made (ADR-084).
    fn estimate(&self, _ctx: &Ctx, _upstream: &Upstream) -> Estimate {
        Estimate::of(self.cost())
    }
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

/// Which tests a runner runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestSelection {
    /// The whole suite.
    All,
    /// These tests, by the IDs the runner's `TestResults` use (ADR-075).
    Tests(Vec<String>),
    /// The tests in these files, relative to the repository root.
    Files(Vec<String>),
    /// The whole suite but these tests, by the IDs the runner's `TestResults` use (ADR-147).
    Except(Vec<String>),
}

/// A command a person or agent runs to run a selection in one project root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerCommand {
    /// The project root it runs in, relative to the repository root; `.` for the root.
    pub dir: String,
    /// Shell words, ready to paste in `dir`.
    pub command: String,
}

/// How a runner runs a selection beyond which tests: added environment, a timeout of its own,
/// and the order, seed, and parallelism a rerun varies (ADR-078, ADR-080). The default is the
/// baseline's run: the runner's own order, seed, and parallelism, and no timeout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    /// Variables added to the runner's environment.
    pub env: Vec<(String, String)>,
    /// Arguments appended to the runner's command, after those that narrow and vary it, such as
    /// the configuration file a property run points the runner at (ADR-095 §3).
    pub args: Vec<String>,
    /// Ends only this run, which then reports an incomplete invocation.
    pub timeout: Option<Duration>,
    /// The most resident memory the run's processes may hold together, in bytes; over it, the
    /// run is killed and is `Error::OutOfMemory` (ADR-148). `None` for no bound.
    pub memory: Option<u64>,
    /// `Shuffled` asks the runner for its seeded shuffle; `Declared` and `Reversed` run a
    /// `Tests` selection in the order given where the runner takes an explicit order.
    pub order: RunOrder,
    /// Handed to the runner as its shuffle seed and, where it has one, its hash seed; `None`
    /// leaves the runner's own.
    pub seed: Option<u64>,
    /// Workers or threads; `None` for the runner's default.
    pub parallelism: Option<u32>,
    /// A build the runner made in another copy of the tree (`Runner::build`) that this run runs
    /// from instead of building, with its own copy as the working tree (ADR-150 §8); `None` to
    /// build where it runs.
    pub prebuilt: Option<Prebuilt>,
}

/// A runner's record of a build it made once for runs in other copies of the tree (ADR-150 §8):
/// per project root, the files the runner wrote describing what it built. Only the runner that
/// made it reads them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prebuilt {
    /// Each project root (relative to the repository root, `.` for the root) and its records.
    pub roots: Vec<(String, Vec<PathBuf>)>,
}

/// What building once made (`Runner::build`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildOutcome {
    /// The build, for runs in other copies of the tree.
    Built(Prebuilt),
    /// It did not build: what the build printed, where a compiler names its errors.
    Failed { output: String },
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            env: Vec::new(),
            args: Vec::new(),
            timeout: None,
            memory: None,
            order: RunOrder::Declared,
            seed: None,
            parallelism: None,
            prebuilt: None,
        }
    }
}

/// What one run of a runner reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRun {
    pub results: TestResults,
    /// Test IDs in the order they ran, when the runner reports it.
    pub executed: Option<Vec<String>>,
}

/// How a runner orders the tests it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOrdering {
    /// In an order nothing changes, such as one process per test (`cargo nextest`).
    Fixed,
    /// In its own order, or a shuffle from a seed.
    Seeded,
    /// In the order of the test IDs it is given (`pytest`, `unittest`).
    Explicit,
}

/// A test runner: produces `TestResults` for the whole suite, and runs selections of it for the
/// consumers that choose tests (`tests --impacted`, the flake and mutation engines).
pub trait Runner: Producer<TestResults> {
    /// The IDs of the suite's tests, without running them.
    fn list(&self, ctx: &ProduceCtx) -> Result<Vec<String>>;

    /// Runs `selection` as `options` say. The run's cancellation still applies
    /// (`Error::Cancelled`).
    fn run(
        &self,
        ctx: &ProduceCtx,
        selection: &TestSelection,
        options: &RunOptions,
    ) -> Result<TestRun>;

    /// The command lines that run `selection`, one per project root it reaches; `None` when the
    /// runner cannot narrow its suite to it.
    fn commands(&self, ctx: &ProduceCtx, selection: &TestSelection) -> Option<Vec<RunnerCommand>>;

    /// How the runner orders what it runs.
    fn ordering(&self) -> RunOrdering;

    /// The names of the directories the runner builds into (such as `target`), which a copy of
    /// the tree must not share with the original or another copy (ADR-080 §8).
    fn outputs(&self) -> &'static [&'static str] {
        &[]
    }

    /// Builds the suite once in `ctx`'s tree, as `options` allow (its parallelism and memory),
    /// for runs in other copies of the tree to run from (`RunOptions::prebuilt`, ADR-150 §8);
    /// `None` when the runner always builds in the tree it runs in.
    fn build(&self, ctx: &ProduceCtx, options: &RunOptions) -> Result<Option<BuildOutcome>> {
        let _ = (ctx, options);
        Ok(None)
    }

    /// Whether `build` makes a build that runs elsewhere run from; a copy of the tree with no
    /// build of its own then builds everything to run a mutant there (ADR-150 §8).
    fn builds_once(&self) -> bool {
        false
    }
}

/// A producer as the registry holds it, with the fact type erased.
pub trait RegisteredProducer: Send + Sync {
    fn provides(&self) -> FactKind;
    fn id(&self) -> ProducerId;
    fn languages(&self) -> Langs;
    fn needs(&self) -> &[FactKind];
    fn wants(&self) -> &[FactKind];
    fn cost(&self) -> CostClass;
    fn estimate(&self, ctx: &Ctx, upstream: &Upstream) -> Estimate;
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

    fn wants(&self) -> &[FactKind] {
        self.producer.wants()
    }

    fn cost(&self) -> CostClass {
        self.producer.cost()
    }

    fn estimate(&self, ctx: &Ctx, upstream: &Upstream) -> Estimate {
        self.producer.estimate(ctx, upstream)
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

/// One run that makes several facts at once, standing in for the per-kind producers that would
/// make them in a run each (ADR-084). Joint producers are kept apart from the per-kind lists:
/// they are never a candidate for one fact, and the support matrix never sees them; the fact
/// store uses one only where it would have chosen exactly the producers it stands in for.
pub trait JointProducer: Send + Sync {
    /// Such as `tool.llvm-cov-nextest`.
    fn id(&self) -> ProducerId;
    fn languages(&self) -> Langs;
    /// Each fact kind it makes, with the ID of the producer it stands in for.
    fn makes(&self) -> &[(FactKind, &'static str)];
    /// Facts it consumes, made first for the same language.
    fn needs(&self) -> &[FactKind];
    /// Facts it reads when the run can afford them, made first for the same language and
    /// handed to it in `upstream` (ADR-149 §3); absent when it cannot.
    fn wants(&self) -> &[FactKind];
    /// What producing here would cost now, for a run focused as `focus` says: its note is
    /// `cached` when it would run nothing, `incremental: N stale` when it would rerun `N` tests
    /// and keep the rest (ADR-149 §6), else `one run`.
    fn estimate(&self, ctx: &Ctx, upstream: &Upstream, focus: &Focus) -> Estimate;
    fn availability(&self, ctx: &Ctx) -> Availability;
    /// The facts of one run; it may make fewer kinds than [`makes`](Self::makes) names, such
    /// as `TestResults` alone for a build that failed.
    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        sink: &mut dyn EventSink,
    ) -> Result<Vec<Produced>>;
    /// Builds ahead of a run what the next run in `ctx`'s repository would build first, when
    /// one of `changed` (absolute paths) is a file its runs depend on, so that run finds it
    /// built (ADR-155 §6), as the daemon does after changes it watches; whether it built. It
    /// makes no fact and changes nothing a run reports.
    fn warm(&self, ctx: &ProduceCtx, changed: &[PathBuf]) -> Result<bool>;
}

/// Every registered producer, by the fact kind it provides, in registration order.
#[derive(Default)]
pub struct ProducerRegistry {
    by_kind: BTreeMap<FactKind, Vec<Box<dyn RegisteredProducer>>>,
    /// The `TestResults` producers registered as runners, by ID (ADR-077).
    runners: BTreeMap<ProducerId, Arc<dyn Runner>>,
    /// The joint producers, in registration order (ADR-084).
    joints: Vec<Box<dyn JointProducer>>,
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

    /// Registers a producer whose fact type is known only at run time, such as a producer plugin,
    /// whose fact its kind's catalog entry decodes (ADR-123).
    pub fn register_dyn(&mut self, producer: Box<dyn RegisteredProducer>) {
        self.by_kind
            .entry(producer.provides())
            .or_default()
            .push(producer);
    }

    /// Registers `runner` as a `TestResults` producer and keeps it for [`runner`](Self::runner).
    pub fn register_runner<R: Runner + Clone + 'static>(&mut self, runner: R) {
        self.runners.insert(runner.id(), Arc::new(runner.clone()));
        self.register::<TestResults, _>(runner);
    }

    /// The runner registered as `id`.
    pub fn runner(&self, id: &ProducerId) -> Option<&dyn Runner> {
        self.runners.get(id).map(AsRef::as_ref)
    }

    /// Registers `joint`, which makes several facts in one run (ADR-084).
    pub fn register_joint<J: JointProducer + 'static>(&mut self, joint: J) {
        self.joints.push(Box::new(joint));
    }

    /// The joint producers that serve `lang`, in registration order.
    pub fn joints_for<'a>(
        &'a self,
        lang: &Lang,
    ) -> impl Iterator<Item = &'a dyn JointProducer> + use<'a> {
        let lang = lang.clone();
        self.joints
            .iter()
            .map(AsRef::as_ref)
            .filter(move |joint| joint.languages().covers(&lang))
    }

    /// Every registered runner, to hand to producers.
    pub fn runners(&self) -> Runners {
        Runners(Arc::new(self.runners.clone()))
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
