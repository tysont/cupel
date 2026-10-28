// ABOUTME: Joint producers (ADR-084, ADR-149) over synthetic producers: one run makes several facts when it saves a run,
// ABOUTME: the per-kind producers run otherwise, pins win, a red joint baseline still refuses, and dry runs say which.
mod support;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use cupel_core::coverage::CoverageMap;
use cupel_core::syntax::Symbols;
use cupel_core::{
    Availability, Check, CheckCtx, CheckRegistry, CheckResult, CheckStatus, CostClass, Ctx,
    Estimate, EventSink, FactKind, Facts, Focus, JointProducer, LangId, Langs, Manifest,
    PerTestMap, Plan, ProduceCtx, Produced, Producer, ProducerId, ProducerRegistry, Result,
    RunnerInvocation, Scheduler, Scope, TestOutcome, TestResult, TestResults, Upstream, Writer,
};

/// How many times each producer ran.
type Runs = Arc<Mutex<BTreeMap<&'static str, u32>>>;

fn ran(runs: &Runs, id: &'static str) {
    *runs.lock().unwrap().entry(id).or_default() += 1;
}

/// A file in the repository that marks a producer's cache as fresh: its estimate is `cached`.
fn cached_in(ctx: &Ctx, id: &str) -> bool {
    ctx.root.join(format!("{id}.cached")).exists()
}

/// One test, `t`, with `outcome`.
fn results(id: &str, outcome: TestOutcome, root: &std::path::Path) -> TestResults {
    TestResults::new(
        ProducerId::from(id),
        vec![RunnerInvocation {
            dir: ".".to_owned(),
            command: vec![id.to_owned()],
            exit_code: Some(0),
            completed: true,
            output: None,
        }],
        vec![TestResult {
            id: "t".to_owned(),
            file: None,
            name: "t".to_owned(),
            outcome,
            duration_ms: Some(1),
            output: None,
        }],
        root,
    )
}

/// A per-kind producer of `F` for rust that counts its runs; `cost` and its cache marker decide
/// its estimate.
struct Single<F> {
    id: &'static str,
    cost: CostClass,
    runs: Runs,
    make: fn(&ProduceCtx) -> F,
}

impl<F: cupel_core::Fact> Producer<F> for Single<F> {
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

    fn estimate(&self, ctx: &Ctx, _: &Upstream) -> Estimate {
        if cached_in(ctx, self.id) {
            Estimate {
                cost: CostClass::Standard,
                note: Some("cached".to_owned()),
            }
        } else {
            Estimate::of(self.cost)
        }
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<F> {
        ran(&self.runs, self.id);
        Ok((self.make)(ctx))
    }
}

/// The joint producer: all three facts, standing in for `tool.runner`, `tool.cover`, and
/// `tool.per-test`; its baseline is red when the repository holds a file named `red`, and it
/// estimates an incremental run on a range when the repository holds a file named
/// `incremental` (ADR-149). It wants `Symbols` and records whether its run saw them.
struct Joint {
    runs: Runs,
    saw_symbols: Arc<Mutex<Option<bool>>>,
}

const MAKES: &[(FactKind, &str)] = &[
    (FactKind::TestResults, "tool.runner"),
    (FactKind::CoverageMap, "tool.cover"),
    (FactKind::PerTestMap, "tool.per-test"),
];

impl JointProducer for Joint {
    fn id(&self) -> ProducerId {
        ProducerId::from("tool.joint")
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![LangId::from("rust")])
    }

    fn makes(&self) -> &[(FactKind, &'static str)] {
        MAKES
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn wants(&self) -> &[FactKind] {
        &[FactKind::Symbols]
    }

    fn estimate(&self, ctx: &Ctx, _: &Upstream, focus: &Focus) -> Estimate {
        let incremental = focus.changed.is_some() && ctx.root.join("incremental").exists();
        Estimate {
            cost: CostClass::Standard,
            note: Some(if cached_in(ctx, "tool.joint") {
                "cached".to_owned()
            } else if incremental {
                "incremental: 1 stale".to_owned()
            } else {
                "one run".to_owned()
            }),
        }
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<Vec<Produced>> {
        ran(&self.runs, "tool.joint");
        *self.saw_symbols.lock().unwrap() = Some(ctx.upstream.get::<Symbols>().is_some());
        let root = &ctx.ctx.root;
        let outcome = if root.join("red").exists() {
            TestOutcome::Failed
        } else {
            TestOutcome::Passed
        };
        Ok(vec![
            Produced::new(results("tool.runner", outcome, root)),
            Produced::new(CoverageMap::default()),
            Produced::new(PerTestMap::new(ProducerId::from("tool.runner"), Vec::new())),
        ])
    }
}

/// An engine that needs `TestResults` and `CoverageMap` and wants `PerTestMap`, standing in for
/// the mutation engine; it records whether it got a per-test map.
struct Engine {
    got_map: Arc<Mutex<Option<bool>>>,
}

impl Producer<cupel_core::MutationOutcomes> for Engine {
    fn id(&self) -> ProducerId {
        ProducerId::from("engine.test")
    }

    fn languages(&self) -> Langs {
        Langs::Any
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::TestResults, FactKind::CoverageMap]
    }

    fn wants(&self) -> &[FactKind] {
        &[FactKind::PerTestMap]
    }

    fn cost(&self) -> CostClass {
        CostClass::Standard
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        _: &Scope,
        _: &mut dyn EventSink,
    ) -> Result<cupel_core::MutationOutcomes> {
        *self.got_map.lock().unwrap() = Some(ctx.upstream.get::<PerTestMap>().is_some());
        Ok(cupel_core::MutationOutcomes::new(
            self.id(),
            None,
            ctx.focus.depth,
            false,
            Vec::new(),
        ))
    }
}

/// A check over `needs` that passes.
struct Passes(Manifest);

impl Check for Passes {
    fn manifest(&self) -> &Manifest {
        &self.0
    }

    fn plan(&self, _: &CheckCtx, _: &Scope) -> Result<Plan> {
        Ok(Plan::estimate(CostClass::Quick, 1))
    }

    fn run(&self, _: &CheckCtx, _: &Facts, _: &Plan, _: &mut dyn EventSink) -> Result<CheckResult> {
        Ok(CheckResult {
            id: self.0.id.clone(),
            status: CheckStatus::Pass,
            tier: self.0.tier,
            cost: self.0.cost,
            score: None,
            metrics: BTreeMap::new(),
            delta: None,
            findings: 0,
            duration_ms: 0,
            skipped: None,
            oracle: None,
            notes: Vec::new(),
        })
    }
}

/// A repository for one scenario: markers (cache files, `red`), configuration, whether the run
/// is a range, and what ran.
struct World {
    dir: tempfile::TempDir,
    toml: String,
    ranged: bool,
    runs: Runs,
    got_map: Arc<Mutex<Option<bool>>>,
    saw_symbols: Arc<Mutex<Option<bool>>>,
}

impl World {
    fn new(markers: &[&str]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        for marker in markers {
            std::fs::write(dir.path().join(marker), "").unwrap();
        }
        Self {
            dir,
            toml: String::new(),
            ranged: false,
            runs: Runs::default(),
            got_map: Arc::default(),
            saw_symbols: Arc::default(),
        }
    }

    fn registry(&self) -> ProducerRegistry {
        let mut producers = ProducerRegistry::default();
        let runs = &self.runs;
        let single = |id, cost| (id, cost, Arc::clone(runs));
        let (id, cost, runs_of) = single("tool.runner", CostClass::Standard);
        producers.register::<TestResults, _>(Single {
            id,
            cost,
            runs: runs_of,
            make: |ctx| results("tool.runner", TestOutcome::Passed, &ctx.ctx.root),
        });
        let (id, cost, runs_of) = single("tool.other-runner", CostClass::Standard);
        producers.register::<TestResults, _>(Single {
            id,
            cost,
            runs: runs_of,
            make: |ctx| results("tool.other-runner", TestOutcome::Passed, &ctx.ctx.root),
        });
        let (id, cost, runs_of) = single("tool.cover", CostClass::Standard);
        producers.register::<CoverageMap, _>(Single {
            id,
            cost,
            runs: runs_of,
            make: |_| CoverageMap::default(),
        });
        let (id, cost, runs_of) = single("tool.per-test", CostClass::Deep);
        producers.register::<PerTestMap, _>(Single {
            id,
            cost,
            runs: runs_of,
            make: |_| PerTestMap::new(ProducerId::from("tool.runner"), Vec::new()),
        });
        producers.register::<cupel_core::MutationOutcomes, _>(Engine {
            got_map: Arc::clone(&self.got_map),
        });
        let (id, cost, runs_of) = single("tool.symbols", CostClass::Quick);
        producers.register::<Symbols, _>(Single {
            id,
            cost,
            runs: runs_of,
            make: |_| Symbols::default(),
        });
        producers.register_joint(Joint {
            runs: Arc::clone(&self.runs),
            saw_symbols: Arc::clone(&self.saw_symbols),
        });
        producers
    }

    /// Runs (or plans, for a dry run) one check needing `needs`; what each producer ran, and
    /// the outcome.
    fn run(
        &self,
        needs: &[FactKind],
        dry_run: bool,
    ) -> (BTreeMap<&'static str, u32>, cupel_core::Outcome) {
        let config = support::config_at(self.dir.path(), &self.toml);
        let writer = Writer::new(self.dir.path(), false).unwrap();
        let producers = self.registry();
        let mut checks = CheckRegistry::default();
        checks
            .register(Box::new(Passes(support::manifest(
                "joint.check",
                needs,
                &[],
            ))))
            .unwrap();
        let mut job = support::job(&writer, &config, &checks, &["rust"]);
        if self.ranged {
            job.changed = Some(BTreeMap::from([("a.x".to_owned(), vec![(1, 1)])]));
        }
        let scheduler = Scheduler {
            checks: &checks,
            producers: &producers,
        };
        let outcome = if dry_run {
            scheduler.plan(&job, &mut Vec::new())
        } else {
            scheduler.run(&job, &mut Vec::new())
        }
        .unwrap();
        (self.runs.lock().unwrap().clone(), outcome)
    }

    fn runs(&self, needs: &[FactKind]) -> BTreeMap<&'static str, u32> {
        self.run(needs, false).0
    }

    fn planned(&self, needs: &[FactKind]) -> Vec<String> {
        self.run(needs, true).1.plan.unwrap().would_run[0]
            .needs
            .clone()
    }
}

fn counts(runs: &[(&'static str, u32)]) -> BTreeMap<&'static str, u32> {
    runs.iter().copied().collect()
}

const BOTH: &[FactKind] = &[FactKind::TestResults, FactKind::CoverageMap];

#[test]
fn one_joint_run_makes_the_facts_two_separate_runs_would_after_making_its_wants() {
    let world = World::new(&[]);
    assert_eq!(
        (world.runs(BOTH), *world.saw_symbols.lock().unwrap()),
        (
            counts(&[("tool.joint", 1), ("tool.symbols", 1)]),
            Some(true)
        )
    );
}

#[test]
fn the_per_kind_producers_run_when_the_joint_would_save_no_run() {
    let cached_coverage = World::new(&["tool.cover.cached"]);
    let one_fact = World::new(&[]);
    assert_eq!(
        (
            cached_coverage.runs(BOTH),
            one_fact.runs(&[FactKind::TestResults])
        ),
        (
            counts(&[("tool.cover", 1), ("tool.runner", 1), ("tool.symbols", 1)]),
            counts(&[("tool.runner", 1), ("tool.symbols", 1)])
        )
    );
}

#[test]
fn a_cached_joint_serves_a_run_that_needs_only_one_of_its_facts() {
    assert_eq!(
        World::new(&["tool.joint.cached"]).runs(&[FactKind::TestResults]),
        counts(&[("tool.joint", 1), ("tool.symbols", 1)])
    );
}

#[test]
fn a_pin_on_another_producer_keeps_the_separate_runs() {
    let mut world = World::new(&[]);
    world.toml = "[lang.rust]\nrunner = \"other-runner\"\n".to_owned();
    assert_eq!(
        world.runs(BOTH),
        counts(&[
            ("tool.cover", 1),
            ("tool.other-runner", 1),
            ("tool.symbols", 1)
        ])
    );
}

#[test]
fn a_red_baseline_from_a_joint_run_refuses_the_run() {
    let (runs, outcome) = World::new(&["red"]).run(BOTH, false);
    assert_eq!(
        (
            runs,
            outcome.checks.len(),
            outcome.refused.map(|red| red.lang)
        ),
        (
            counts(&[("tool.joint", 1), ("tool.symbols", 1)]),
            0,
            Some(LangId::from("rust"))
        )
    );
}

#[test]
fn a_want_the_joint_run_made_reaches_the_producer_beyond_the_depth_its_own_producer_costs() {
    let world = World::new(&[]);
    let runs = world.runs(&[FactKind::MutationOutcomes]);
    assert_eq!(
        (runs, *world.got_map.lock().unwrap()),
        (
            counts(&[("tool.joint", 1), ("tool.symbols", 1)]),
            Some(true)
        )
    );
}

#[test]
fn a_dry_run_plans_one_joint_run_or_says_how_each_fact_would_be_made() {
    let mutation = &[FactKind::MutationOutcomes];
    assert_eq!(
        [
            World::new(&[]).planned(mutation),
            World::new(&["tool.joint.cached"]).planned(mutation),
            World::new(&["tool.cover.cached"]).planned(mutation),
        ],
        [
            vec![
                "mutation-outcomes",
                "test-results+coverage-map+per-test-map(one run)"
            ],
            vec![
                "mutation-outcomes",
                "test-results+coverage-map+per-test-map(cached)"
            ],
            vec![
                "mutation-outcomes",
                "test-results(run)",
                "coverage-map(cached)",
                "per-test-map(deep)"
            ],
        ]
    );
}

#[test]
fn a_dry_run_on_a_range_says_how_many_tests_an_incremental_joint_run_would_rerun() {
    let mut ranged = World::new(&["incremental"]);
    ranged.ranged = true;
    let tree = World::new(&["incremental"]);
    let mutation = &[FactKind::MutationOutcomes];
    assert_eq!(
        [ranged.planned(mutation), tree.planned(mutation)],
        [
            vec![
                "mutation-outcomes",
                "test-results+coverage-map+per-test-map(incremental: 1 stale)"
            ],
            vec![
                "mutation-outcomes",
                "test-results+coverage-map+per-test-map(one run)"
            ],
        ]
    );
}
