// ABOUTME: Tests for the extension model: fact kinds, the `Facts` accessor, language coverage,
// ABOUTME: check and producer registries, and support-matrix resolution over synthetic registries.
mod support;

use std::marker::PhantomData;
use std::path::PathBuf;

use cupel_core::syntax::{Decisions, MutationSites, Symbols};
use cupel_core::{
    ActionKind, Availability, Cancellation, Check, CheckCtx, CheckId, CheckRegistry, CheckResult,
    CheckStatus, CostClass, Ctx, Error, Event, EventSink, Fact, FactKind, Facts, FindingKindSpec,
    Lang, LangFamily, LangId, Langs, Manifest, MetricPer, MetricSpec, Plan, ProduceCtx, Producer,
    ProducerId, ProducerRegistry, Result, RunId, Scope, Support, SupportMatrix, ToolId, WriteError,
    Writer,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

// ---------------------------------------------------------------- fixtures
fn rust() -> Lang {
    support::lang("rust", "rust")
}

fn go() -> Lang {
    support::lang("go", "go")
}

fn javascript() -> Lang {
    support::lang("javascript", "ecma")
}

fn typescript() -> Lang {
    support::lang("typescript", "ecma")
}

fn ctx() -> Ctx {
    Ctx::new(PathBuf::from("."))
}
/// A check that reports one result and never reads facts.
struct ReportingCheck(Manifest);

impl Check for ReportingCheck {
    fn manifest(&self) -> &Manifest {
        &self.0
    }

    fn plan(&self, _: &CheckCtx, scope: &Scope) -> Result<Plan> {
        Ok(Plan {
            targets: scope.files.len() as u64,
            est_ms: 0,
        })
    }

    fn run(&self, _: &CheckCtx, _: &Facts, _: &Plan, _: &mut dyn EventSink) -> Result<CheckResult> {
        Ok(CheckResult {
            id: self.0.id.clone(),
            status: CheckStatus::Pass,
            tier: self.0.tier,
            cost: self.0.cost,
            score: None,
            metrics: std::collections::BTreeMap::new(),
            delta: None,
            findings: 0,
            duration_ms: 0,
            skipped: None,
            oracle: None,
            notes: Vec::new(),
        })
    }
}

fn check(id: &str, needs: &[FactKind], wants: &[FactKind]) -> Box<dyn Check> {
    Box::new(ReportingCheck(support::manifest(id, needs, wants)))
}

/// An execution fact standing in for `MutationOutcomes`, whose payload lands in P3.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
struct Outcomes {
    killed: u32,
}

impl Fact for Outcomes {
    const KIND: FactKind = FactKind::MutationOutcomes;
    const VERSION: u32 = 1;
}

/// A producer with configurable coverage, needs, and availability that yields a fixed fact.
struct FixedProducer<F> {
    id: &'static str,
    languages: Langs,
    needs: Vec<FactKind>,
    availability: Availability,
    fact: PhantomData<fn() -> F>,
}

fn producer<F>(id: &'static str, languages: Langs, needs: &[FactKind]) -> FixedProducer<F> {
    FixedProducer {
        id,
        languages,
        needs: needs.to_vec(),
        availability: Availability::Ready,
        fact: PhantomData,
    }
}

impl<F> FixedProducer<F> {
    fn with(mut self, availability: Availability) -> Self {
        self.availability = availability;
        self
    }
}

impl<F: Fact + Default> Producer<F> for FixedProducer<F> {
    fn id(&self) -> ProducerId {
        ProducerId::from(self.id)
    }

    fn languages(&self) -> Langs {
        self.languages.clone()
    }

    fn needs(&self) -> &[FactKind] {
        &self.needs
    }

    fn cost(&self) -> CostClass {
        CostClass::Quick
    }

    fn availability(&self, _: &Ctx) -> Availability {
        self.availability.clone()
    }

    fn produce(&self, _: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<F> {
        Ok(F::default())
    }
}

fn only(langs: &[&str]) -> Langs {
    Langs::Only(langs.iter().map(|l| LangId::from(*l)).collect())
}

fn matrix(checks: &CheckRegistry, producers: &ProducerRegistry, langs: &[Lang]) -> SupportMatrix {
    SupportMatrix::compute(checks, producers, langs, &ctx())
}

fn checks(list: Vec<Box<dyn Check>>) -> CheckRegistry {
    let mut registry = CheckRegistry::default();
    for check in list {
        registry.register(check).unwrap();
    }
    registry
}

fn support(m: &SupportMatrix, check: &str, lang: &Lang) -> Support {
    m.get(&CheckId::from(check), &lang.id)
        .cloned()
        .expect("every (check, lang) pair has a cell")
}

// ---------------------------------------------------------------- fact kinds

#[test]
fn fact_kinds_are_the_catalog_rows_in_kebab_case() {
    let names = [
        "symbols",
        "decisions",
        "norm-tree",
        "mutation-sites",
        "test-frames",
        "module-graph",
        "api-surface",
        "roots",
        "test-conventions",
        "project-info",
        "test-results",
        "test-reruns",
        "coverage-map",
        "per-test-map",
        "mutation-outcomes",
        "bench-samples",
        "property-results",
        "fuzz-results",
        "race-reports",
        "analyzer-report",
        "divergence-results",
    ];
    let spelled: Vec<(serde_json::Value, String, &str)> = FactKind::ALL
        .into_iter()
        .map(|kind| {
            (
                serde_json::to_value(kind).unwrap(),
                kind.to_string(),
                kind.as_str(),
            )
        })
        .collect();
    let expected: Vec<(serde_json::Value, String, &str)> = names
        .into_iter()
        .map(|name| (json!(name), name.to_owned(), name))
        .collect();
    assert_eq!(spelled, expected);
}

// ---------------------------------------------------------------- Facts accessor

#[test]
fn facts_serves_a_declared_fact_that_was_inserted() {
    let mut facts = Facts::for_manifest(&support::manifest("c", &[FactKind::Symbols], &[]));
    facts.insert(Symbols { symbols: vec![] }).unwrap();
    assert_eq!(
        facts.get::<Symbols>().unwrap(),
        &Symbols { symbols: vec![] }
    );
}

#[test]
fn facts_get_of_an_undeclared_kind_is_an_error() {
    let facts = Facts::for_manifest(&support::manifest("c", &[FactKind::Symbols], &[]));
    assert_eq!(
        facts.get::<Decisions>().unwrap_err(),
        Error::UndeclaredFact {
            kind: FactKind::Decisions
        }
    );
}

#[test]
fn facts_get_of_a_declared_but_absent_kind_is_an_error() {
    let facts = Facts::for_manifest(&support::manifest("c", &[FactKind::Symbols], &[]));
    assert_eq!(
        facts.get::<Symbols>().unwrap_err(),
        Error::AbsentFact {
            kind: FactKind::Symbols
        }
    );
}

#[test]
fn facts_refuses_to_hold_an_undeclared_kind() {
    let mut facts = Facts::for_manifest(&support::manifest("c", &[FactKind::Symbols], &[]));
    assert_eq!(
        facts.insert(Decisions { symbols: vec![] }).unwrap_err(),
        Error::UndeclaredFact {
            kind: FactKind::Decisions
        }
    );
}

#[test]
fn facts_serves_wanted_kinds_too() {
    let mut facts = Facts::for_manifest(&support::manifest(
        "c",
        &[FactKind::Symbols],
        &[FactKind::Decisions],
    ));
    facts.insert(Decisions { symbols: vec![] }).unwrap();
    assert_eq!(facts.get::<Decisions>(), Ok(&Decisions { symbols: vec![] }));
}

// ---------------------------------------------------------------- languages and sinks

#[test]
fn langs_cover_by_any_by_id_and_by_family() {
    let ecma = Langs::Family(LangFamily::from("ecma"));
    let cases = [
        (Langs::Any, go()),
        (only(&["rust", "go"]), go()),
        (only(&["rust"]), go()),
        (ecma.clone(), javascript()),
        (ecma.clone(), typescript()),
        (ecma, rust()),
    ];
    let covered: Vec<bool> = cases
        .iter()
        .map(|(langs, lang)| langs.covers(lang))
        .collect();
    assert_eq!(covered, [true, true, false, true, true, false]);
}

#[test]
fn a_vec_collects_events_in_order() {
    let mut sink: Vec<Event> = Vec::new();
    let first = Event::Skip {
        id: CheckId::from("a"),
        reason: cupel_core::SkipReason::Budget,
    };
    let second = Event::CheckStart {
        id: CheckId::from("b"),
        targets: 1,
    };
    let dyn_sink: &mut dyn EventSink = &mut sink;
    dyn_sink.emit(first.clone());
    dyn_sink.emit(second.clone());
    assert_eq!(sink, vec![first, second]);
}

// ---------------------------------------------------------------- registries

#[test]
fn check_registry_finds_checks_by_id_and_rejects_duplicates() {
    let mut registry = checks(vec![check("static.complexity", &[], &[])]);
    let found = registry.get(&CheckId::from("static.complexity")).unwrap();
    assert_eq!(found.manifest().id, CheckId::from("static.complexity"));
    assert!(registry.get(&CheckId::from("static.clones")).is_none());
    assert_eq!(
        registry
            .register(check("static.complexity", &[], &[]))
            .unwrap_err(),
        Error::DuplicateCheck {
            id: "static.complexity".to_owned()
        }
    );
    assert_eq!(registry.iter().count(), 1);
}

#[test]
fn producer_registry_selects_by_kind_and_language_in_registration_order() {
    let mut registry = ProducerRegistry::default();
    registry.register(producer::<Symbols>("lang.rust", only(&["rust"]), &[]));
    registry.register(producer::<Symbols>("lang.any", Langs::Any, &[]));
    registry.register(producer::<Decisions>("lang.decisions", Langs::Any, &[]));
    let ids = |kind, lang: &Lang| -> Vec<String> {
        registry
            .producers_for(kind, lang)
            .map(|p| p.id().to_string())
            .collect()
    };
    assert_eq!(ids(FactKind::Symbols, &rust()), ["lang.rust", "lang.any"]);
    assert_eq!(ids(FactKind::Symbols, &go()), ["lang.any"]);
    assert_eq!(ids(FactKind::Decisions, &go()), ["lang.decisions"]);
    assert!(ids(FactKind::NormTree, &go()).is_empty());
    let registered = registry
        .producers_for(FactKind::Decisions, &go())
        .next()
        .unwrap();
    assert_eq!(registered.provides(), FactKind::Decisions);
    assert_eq!(registered.cost(), CostClass::Quick);
}

#[test]
fn manifest_round_trips_with_kebab_fact_kinds() {
    let mut m = support::manifest(
        "tests.mutation",
        &[FactKind::MutationOutcomes],
        &[FactKind::TestConventions],
    );
    m.findings.push(FindingKindSpec {
        kind: "mutant.survived".into(),
        action: ActionKind::WriteTest,
        summary: "A mutant no test killed.".into(),
        evidence: serde_json::Map::from_iter([("type".to_owned(), json!("object"))]),
    });
    m.metrics.push(MetricSpec {
        name: "score".into(),
        summary: "Killed ratio.".into(),
        per: MetricPer::Scope,
    });
    let value = serde_json::to_value(&m).unwrap();
    assert_eq!(value["needs"], json!(["mutation-outcomes"]));
    assert_eq!(value["wants"], json!(["test-conventions"]));
    assert_eq!(value["stability"], json!("experimental"));
    assert_eq!(value["findings"][0]["action"], json!("write_test"));
    assert_eq!(serde_json::from_value::<Manifest>(value).unwrap(), m);
}

// ---------------------------------------------------------------- support matrix

/// `static.complexity`, needing symbols and decisions, and a producer of symbols for any language.
fn complexity_with_symbols() -> (CheckRegistry, ProducerRegistry) {
    let checks = checks(vec![check(
        "static.complexity",
        &[FactKind::Symbols, FactKind::Decisions],
        &[],
    )]);
    let mut producers = ProducerRegistry::default();
    producers.register(producer::<Symbols>("lang.symbols", Langs::Any, &[]));
    (checks, producers)
}

#[test]
fn check_needing_a_fact_without_producer_is_unsupported_for_that_fact() {
    let (checks, producers) = complexity_with_symbols();
    let m = matrix(&checks, &producers, &[rust()]);
    assert_eq!(
        support(&m, "static.complexity", &rust()),
        Support::Unsupported(FactKind::Decisions)
    );
}

#[test]
fn check_is_supported_exactly_where_every_need_has_a_producer() {
    let (checks, mut producers) = complexity_with_symbols();
    producers.register(producer::<Decisions>("lang.rust", only(&["rust"]), &[]));
    let m = matrix(&checks, &producers, &[rust(), go()]);
    assert_eq!(
        support(&m, "static.complexity", &rust()),
        Support::Supported
    );
    assert_eq!(
        support(&m, "static.complexity", &go()),
        Support::Unsupported(FactKind::Decisions)
    );
}

#[test]
fn family_producers_support_every_language_in_the_family() {
    let checks = checks(vec![check("static.clones", &[FactKind::Symbols], &[])]);
    let mut producers = ProducerRegistry::default();
    producers.register(producer::<Symbols>(
        "lang.ecma",
        Langs::Family(LangFamily::from("ecma")),
        &[],
    ));
    let m = matrix(&checks, &producers, &[javascript(), typescript(), rust()]);
    assert_eq!(
        support(&m, "static.clones", &javascript()),
        Support::Supported
    );
    assert_eq!(
        support(&m, "static.clones", &typescript()),
        Support::Supported
    );
    assert_eq!(
        support(&m, "static.clones", &rust()),
        Support::Unsupported(FactKind::Symbols)
    );
}

#[test]
fn producer_missing_its_tool_makes_the_check_unavailable() {
    let checks = checks(vec![check(
        "tests.mutation",
        &[FactKind::MutationOutcomes],
        &[],
    )]);
    let mut producers = ProducerRegistry::default();
    producers.register(producer::<Outcomes>("tool.pit", Langs::Any, &[]).with(
        Availability::MissingTool {
            name: ToolId::from("pitest"),
            install: Some(cupel_core::Install::Command(
                "mvn dependency:get -Dartifact=org.pitest:pitest".into(),
            )),
        },
    ));
    let m = matrix(&checks, &producers, &[rust()]);
    assert_eq!(
        support(&m, "tests.mutation", &rust()),
        Support::Unavailable(ToolId::from("pitest"))
    );
}

#[test]
fn unconfigured_producer_reports_its_config_key() {
    let checks = checks(vec![check(
        "tests.mutation",
        &[FactKind::MutationOutcomes],
        &[],
    )]);
    let mut producers = ProducerRegistry::default();
    producers.register(
        producer::<Outcomes>("engine.mutation", Langs::Any, &[]).with(Availability::Unconfigured {
            key: "lang.rust.test".into(),
        }),
    );
    let m = matrix(&checks, &producers, &[rust()]);
    assert_eq!(
        support(&m, "tests.mutation", &rust()),
        Support::Unconfigured("lang.rust.test".into())
    );
}

#[test]
fn a_ready_producer_later_in_order_still_supports_the_check() {
    let checks = checks(vec![check(
        "tests.mutation",
        &[FactKind::MutationOutcomes],
        &[],
    )]);
    let mut producers = ProducerRegistry::default();
    producers.register(producer::<Outcomes>("tool.pit", Langs::Any, &[]).with(
        Availability::MissingTool {
            name: ToolId::from("pitest"),
            install: None,
        },
    ));
    producers.register(producer::<Outcomes>("engine.mutation", Langs::Any, &[]));
    let m = matrix(&checks, &producers, &[rust()]);
    assert_eq!(support(&m, "tests.mutation", &rust()), Support::Supported);
}

#[test]
fn producer_needs_are_resolved_transitively() {
    let checks = checks(vec![check(
        "tests.mutation",
        &[FactKind::MutationOutcomes],
        &[],
    )]);
    let mut producers = ProducerRegistry::default();
    producers.register(producer::<Outcomes>(
        "engine.mutation",
        Langs::Any,
        &[FactKind::MutationSites],
    ));
    producers.register(producer::<MutationSites>("lang.rust", only(&["rust"]), &[]));
    let m = matrix(&checks, &producers, &[rust(), go()]);
    assert_eq!(support(&m, "tests.mutation", &rust()), Support::Supported);
    assert_eq!(
        support(&m, "tests.mutation", &go()),
        Support::Unsupported(FactKind::MutationSites)
    );
}

#[test]
fn a_missing_upstream_fact_outranks_a_missing_tool() {
    let checks = checks(vec![check(
        "tests.mutation",
        &[FactKind::MutationOutcomes],
        &[],
    )]);
    let mut producers = ProducerRegistry::default();
    producers.register(
        producer::<Outcomes>("tool.pit", Langs::Any, &[FactKind::MutationSites]).with(
            Availability::MissingTool {
                name: ToolId::from("pitest"),
                install: None,
            },
        ),
    );
    let m = matrix(&checks, &producers, &[go()]);
    assert_eq!(
        support(&m, "tests.mutation", &go()),
        Support::Unsupported(FactKind::MutationSites)
    );
}

#[test]
fn a_cycle_among_producers_resolves_as_unsupported() {
    let checks = checks(vec![check("static.complexity", &[FactKind::Symbols], &[])]);
    let mut producers = ProducerRegistry::default();
    producers.register(producer::<Symbols>("a", Langs::Any, &[FactKind::Decisions]));
    producers.register(producer::<Decisions>("b", Langs::Any, &[FactKind::Symbols]));
    let m = matrix(&checks, &producers, &[rust()]);
    assert_eq!(
        support(&m, "static.complexity", &rust()),
        Support::Unsupported(FactKind::Symbols)
    );
}

#[test]
fn wants_never_affect_support() {
    let checks = checks(vec![check(
        "tests.mutation",
        &[FactKind::Symbols],
        &[FactKind::TestConventions],
    )]);
    let mut producers = ProducerRegistry::default();
    producers.register(producer::<Symbols>("lang.symbols", Langs::Any, &[]));
    let m = matrix(&checks, &producers, &[rust()]);
    assert_eq!(support(&m, "tests.mutation", &rust()), Support::Supported);
}

#[test]
fn matrix_has_one_cell_per_check_and_language() {
    let checks = checks(vec![
        check("a", &[FactKind::Symbols], &[]),
        check("b", &[], &[]),
    ]);
    let producers = ProducerRegistry::default();
    let langs = [rust(), go(), typescript()];
    let m = matrix(&checks, &producers, &langs);
    assert_eq!(m.cells().count(), 6);
    assert_eq!(support(&m, "b", &go()), Support::Supported);
    assert_eq!(
        support(&m, "a", &go()),
        Support::Unsupported(FactKind::Symbols)
    );
}

// ---------------------------------------------------------------- run controls in contexts

/// Caches its output under `.cupel/cache/` and tries to write into the repository, stopping
/// first if the run was cancelled.
struct CachingProducer;

impl Producer<Symbols> for CachingProducer {
    fn id(&self) -> ProducerId {
        ProducerId::from("lang.caching")
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
        ctx.cancel.checkpoint()?;
        ctx.writer
            .write(&ctx.writer.cache_dir().join("symbols"), b"cached")?;
        ctx.writer
            .write(std::path::Path::new("src/generated.rs"), b"")?;
        Ok(Symbols::default())
    }
}

fn produce_ctx(root: &std::path::Path, cancel: Cancellation) -> ProduceCtx {
    // `--write` was given, yet producers only ever get the confined writer.
    let writer = Writer::new(root, true).unwrap().confined();
    ProduceCtx::new(Ctx::new(writer.root().to_path_buf()), writer, cancel)
}

#[test]
fn producers_write_only_under_cupel_through_their_context() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = produce_ctx(dir.path(), Cancellation::new());
    let result = CachingProducer.produce(&ctx, &Scope::default(), &mut Vec::new());
    let outside = dir.path().canonicalize().unwrap().join("src/generated.rs");
    assert_eq!(
        result,
        Err(Error::Write(WriteError::Outside { path: outside }))
    );
    assert_eq!(
        support::files_under(dir.path()),
        [".cupel/.gitignore", ".cupel/cache/symbols"]
    );
}

#[test]
fn producers_stop_at_a_checkpoint_once_the_run_is_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = Cancellation::new();
    cancel.cancel();
    let ctx = produce_ctx(dir.path(), cancel);
    let result = CachingProducer.produce(&ctx, &Scope::default(), &mut Vec::new());
    assert_eq!(result, Err(Error::Cancelled));
    assert!(!dir.path().join(".cupel").exists());
}

#[test]
fn a_check_context_carries_a_scratch_writer_for_its_run() {
    let dir = tempfile::tempdir().unwrap();
    let run = RunId::generate();
    let writer = Writer::new(dir.path(), true).unwrap();
    let ctx = CheckCtx {
        scope: Scope::default(),
        config: serde_json::Map::new(),
        writer: writer.scratch(&run),
        cancel: Cancellation::new(),
    };
    assert_eq!(ctx.writer.area(), writer.work_dir(&run));
}

#[test]
fn a_registered_producer_produces_its_fact_with_the_type_erased() {
    let dir = tempfile::tempdir().unwrap();
    let mut registry = ProducerRegistry::default();
    registry.register::<Decisions, _>(producer("lang.rust", only(&["rust"]), &[]));
    let producer = registry
        .producers_for(FactKind::Decisions, &rust())
        .next()
        .unwrap();
    let ctx = produce_ctx(dir.path(), Cancellation::new());

    let fact = producer
        .produce(&ctx, &Scope::default(), &mut Vec::new())
        .unwrap();

    assert_eq!(fact.kind(), FactKind::Decisions);
    assert_eq!(fact.to_json(), json!({ "symbols": [] }));
    assert!(fact.downcast_ref::<Symbols>().is_none());
    assert_eq!(fact.downcast::<Decisions>(), Some(Decisions::default()));
}
