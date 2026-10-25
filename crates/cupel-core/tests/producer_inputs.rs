// ABOUTME: What the scheduler hands a producer beside its needs (ADR-080): the facts it wants when they can be
// ABOUTME: made, the registered runners, the run's depth, the lines a range changed, its oracle, and `--cases`.
mod support;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use cupel_core::config::{OracleConfig, OracleKind};
use cupel_core::coverage::CoverageMap;
use cupel_core::syntax::{ProjectInfo, Roots as EntryPoints, Symbols};
use cupel_core::{
    Availability, BaseTree, CheckRegistry, CostClass, Ctx, Error, Estimate, EventSink, FactKind,
    Focus, Langs, ProduceCtx, Producer, ProducerId, ProducerRegistry, Range, Result, RunId,
    RunOracle, Scheduler, Scope, Upstream, Writer,
};

/// What the recording producer saw: the kinds upstream held and the runners it was handed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Seen {
    project_info: bool,
    coverage: bool,
    roots: bool,
    runners: String,
    focus: Option<Focus>,
    oracle: Option<String>,
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
        &[
            FactKind::ProjectInfo,
            FactKind::CoverageMap,
            FactKind::Roots,
        ]
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
            roots: ctx.upstream.get::<EntryPoints>().is_some(),
            runners: format!("{:?}", ctx.runners),
            focus: Some(ctx.focus.clone()),
            oracle: ctx.oracle.as_ref().map(|oracle| oracle.name.clone()),
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

/// Entry points at `deep` cost: wanted, but made only when the run's depth reaches it, unless
/// a file named `cached` in the repository makes it a `standard` refresh (ADR-083).
struct Expensive;

impl Producer<EntryPoints> for Expensive {
    fn id(&self) -> ProducerId {
        ProducerId::from("tool.expensive")
    }

    fn languages(&self) -> Langs {
        Langs::Any
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn cost(&self) -> CostClass {
        CostClass::Deep
    }

    fn estimate(&self, ctx: &Ctx, _: &Upstream) -> Estimate {
        if ctx.root.join("cached").exists() {
            Estimate {
                cost: CostClass::Standard,
                note: Some("cached".to_owned()),
            }
        } else {
            Estimate::of(self.cost())
        }
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(&self, _: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<EntryPoints> {
        Ok(EntryPoints::default())
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
    seen_at(CostClass::Deep)
}

/// What the recording producer saw in a run at `depth` (see [`seen`]).
fn seen_at(depth: CostClass) -> Seen {
    seen_in(depth, false)
}

/// What the recording producer saw in a run at `depth`, with the expensive producer's cache
/// present when `cached`.
fn seen_in(depth: CostClass, cached: bool) -> Seen {
    let dir = tempfile::tempdir().unwrap();
    if cached {
        std::fs::write(dir.path().join("cached"), "").unwrap();
    }
    let config = support::config_at(dir.path(), "");
    let writer = Writer::new(dir.path(), false).unwrap();
    let recorded = Arc::new(Mutex::new(None));
    let mut producers = ProducerRegistry::default();
    producers.register::<Symbols, _>(Recording(Arc::clone(&recorded)));
    producers.register::<ProjectInfo, _>(Roots);
    producers.register::<CoverageMap, _>(Broken);
    producers.register::<EntryPoints, _>(Expensive);
    let checks = CheckRegistry::default();
    let mut job = support::job(&writer, &config, &checks, &["rust"]);
    job.depth = depth;
    job.changed = Some(changed());
    job.base = Some(BaseTree::planned(
        Range {
            commit: "c0ffee".to_owned(),
            changes: Vec::new(),
        },
        &writer.scratch(&RunId::generate()),
    ));
    job.cases = Some(7);
    job.oracle = Some(RunOracle {
        name: "release".to_owned(),
        definition: OracleConfig {
            kind: OracleKind::Ref,
            reference: Some("v1".to_owned()),
            cmd: None,
            spec: None,
            lang: None,
            inputs: Vec::new(),
        },
    });
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
            base: Some("c0ffee".to_owned()),
            jobs: None,
            cases: Some(7),
        })
    );
}

#[test]
fn a_producer_gets_the_runs_oracle() {
    assert_eq!(seen().oracle.as_deref(), Some("release"));
}

#[test]
fn a_want_only_a_costlier_producer_than_the_runs_depth_makes_is_left_out() {
    assert_eq!(
        (
            seen_at(CostClass::Standard).roots,
            seen_at(CostClass::Deep).roots
        ),
        (false, true)
    );
}

#[test]
fn a_want_whose_producer_estimates_a_cheap_refresh_is_made_at_that_depth() {
    assert_eq!(
        (
            seen_in(CostClass::Standard, true).roots,
            seen_in(CostClass::Quick, true).roots
        ),
        (true, false)
    );
}

/// The `needs` a dry run at `depth` plans for a check needing `Symbols`, whose producer wants
/// the expensive entry points, with that producer's cache present when `cached`.
fn planned_needs(depth: CostClass, cached: bool) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    if cached {
        std::fs::write(dir.path().join("cached"), "").unwrap();
    }
    let config = support::config_at(dir.path(), "");
    let writer = Writer::new(dir.path(), false).unwrap();
    let mut producers = ProducerRegistry::default();
    producers.register::<Symbols, _>(Recording(Arc::new(Mutex::new(None))));
    producers.register::<EntryPoints, _>(Expensive);
    let mut checks = CheckRegistry::default();
    let manifest = support::manifest("needs.symbols", &[FactKind::Symbols], &[]);
    checks
        .register(Box::new(support::Declared(manifest)))
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
fn a_dry_run_says_how_a_want_of_a_needed_facts_producer_would_be_made() {
    assert_eq!(
        [
            planned_needs(CostClass::Standard, false),
            planned_needs(CostClass::Standard, true),
            planned_needs(CostClass::Deep, false),
        ],
        [
            vec!["symbols", "roots(deep)"],
            vec!["symbols", "roots(cached)"],
            vec!["symbols"],
        ]
    );
}

/// Each language a producer ran for, with the settings it got.
type SettingsSeen = Vec<(String, serde_json::Map<String, serde_json::Value>)>;

/// A `Symbols` producer that records, per language, the settings checks declared for it.
struct Configured(Arc<Mutex<SettingsSeen>>);

impl Producer<Symbols> for Configured {
    fn id(&self) -> ProducerId {
        ProducerId::from("lang.configured")
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

    fn produce(&self, ctx: &ProduceCtx, scope: &Scope, _: &mut dyn EventSink) -> Result<Symbols> {
        let lang = scope.files.first().cloned().unwrap_or_default();
        self.0.lock().unwrap().push((lang, ctx.settings.clone()));
        Ok(Symbols::default())
    }
}

/// The settings a `Symbols` producer got for rust and go under `toml`, with a check that
/// declares `cap` (and `plain`, which configures no fact) registered but not selected.
fn settings_seen(toml: &str) -> Vec<(String, serde_json::Value)> {
    let dir = tempfile::tempdir().unwrap();
    let config = support::config_at(dir.path(), toml);
    let writer = Writer::new(dir.path(), false).unwrap();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let mut producers = ProducerRegistry::default();
    producers.register::<Symbols, _>(Configured(Arc::clone(&recorded)));
    let mut checks = CheckRegistry::default();
    let setting = |name: &str, configures| cupel_core::SettingSpec {
        name: name.to_owned(),
        default: serde_json::json!("1s"),
        summary: String::new(),
        configures,
    };
    checks
        .register(Box::new(support::Declared(cupel_core::Manifest {
            settings: vec![
                setting("cap", Some(FactKind::Symbols)),
                setting("plain", None),
            ],
            ..support::manifest("t.cap", &[FactKind::Symbols], &[])
        })))
        .unwrap();
    let mut job = support::job(&writer, &config, &checks, &["rust", "go"]);
    job.selected = Vec::new();
    let scheduler = Scheduler {
        checks: &checks,
        producers: &producers,
    };
    for lang in ["rust", "go"] {
        job.files = vec![lang.to_owned()];
        scheduler
            .production(&job)
            .ensure(
                FactKind::Symbols,
                &support::lang(lang, lang),
                &mut Vec::new(),
            )
            .unwrap();
    }
    let seen = recorded.lock().unwrap().clone();
    seen.into_iter()
        .map(|(lang, settings)| (lang, serde_json::Value::Object(settings)))
        .collect()
}

#[test]
fn a_producer_gets_the_settings_checks_declare_for_its_fact_as_configured_for_its_language() {
    let toml = "[checks.\"t.cap\"]\ncap = \"3s\"\nplain = \"2s\"\n\
                [checks.\"t.cap\".lang.go]\ncap = \"5s\"\n";
    assert_eq!(
        settings_seen(toml),
        [
            ("rust".to_owned(), serde_json::json!({"cap": "3s"})),
            ("go".to_owned(), serde_json::json!({"cap": "5s"})),
        ]
    );
}

#[test]
fn an_unconfigured_setting_for_a_fact_is_left_to_the_producer() {
    assert_eq!(
        settings_seen(""),
        [
            ("rust".to_owned(), serde_json::json!({})),
            ("go".to_owned(), serde_json::json!({})),
        ]
    );
}
