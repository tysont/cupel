// ABOUTME: Scheduler v1 over synthetic registries: facts produced once and served only as declared,
// ABOUTME: unsupported pairs skipped, findings de-duplicated, cancellation, combining, planning (ADR-041).
#![allow(
    clippy::unnecessary_wraps,
    reason = "test checks share the `Check::run` signature, which returns a Result"
)]
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use cupel_core::config::{Config, ConfigRequest};
use cupel_core::syntax::{Decisions, Span, SymbolDef, SymbolKind, Symbols, Visibility};
use cupel_core::{
    Action, ActionKind, Availability, Cancellation, Check, CheckCtx, CheckId, CheckRegistry,
    CheckResult, CheckStatus, CostClass, Ctx, Error, Event, EventSink, Evidence, Fact, FactKind,
    Facts, Finding, FindingId, Job, Lang, LangFamily, LangId, Langs, Location, Manifest, Outcome,
    Plan, ProduceCtx, Producer, ProducerId, ProducerRegistry, Result, RunId, Scheduler, Scope,
    Score, Severity, SkipReason, Stability, SymbolSelector, Tier, Writer, next_actions,
};

// ---------------------------------------------------------------- fixtures

fn lang(id: &str) -> Lang {
    Lang {
        id: LangId::from(id),
        family: LangFamily::from(id),
    }
}

/// A finding about `file`, as producers and checks emit them.
fn finding(check: &str, kind: &str, file: &str, severity: Severity, tier: u8) -> Finding {
    let check = CheckId::from(check);
    let id = FindingId::for_subject(&check, kind, file);
    Finding {
        suppress: String::new(),
        id,
        check,
        kind: kind.to_owned(),
        severity,
        confidence: 1.0,
        tier: Tier::try_from(tier).unwrap(),
        symbol: None,
        location: Some(Location {
            file: file.to_owned(),
            line: 1,
            col: None,
            end_line: 1,
            end_col: None,
        }),
        title: format!("{kind} in {file}"),
        detail: None,
        evidence: Evidence::default(),
        action: Action {
            kind: ActionKind::Review,
            priority: 1,
            summary: format!("look at {file}"),
            hints: serde_json::Map::new(),
        },
        repro: String::new(),
        new_since_baseline: false,
        suppressed: false,
    }
}

/// Makes a default `F` for the languages it serves, counting productions; each production
/// reports a `broken` finding about `broken.x`, the same for every producer.
struct Counting<F> {
    langs: Vec<&'static str>,
    count: Arc<AtomicUsize>,
    make: fn(&Scope) -> F,
}

impl<F: Fact> Producer<F> for Counting<F> {
    fn id(&self) -> ProducerId {
        ProducerId::from("lang.test")
    }

    fn languages(&self) -> Langs {
        Langs::Only(self.langs.iter().map(|l| LangId::from(*l)).collect())
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

    fn produce(&self, ctx: &ProduceCtx, scope: &Scope, sink: &mut dyn EventSink) -> Result<F> {
        ctx.cancel.checkpoint()?;
        self.count.fetch_add(1, Ordering::SeqCst);
        sink.emit(Event::Finding(Box::new(finding(
            "lang",
            "lang.parse-error",
            "broken.x",
            Severity::Warn,
            0,
        ))));
        Ok((self.make)(scope))
    }
}

/// One symbol per file in scope, at line 1.
fn symbols_of(scope: &Scope) -> Symbols {
    Symbols {
        symbols: scope
            .files
            .iter()
            .map(|file| SymbolDef {
                name: file.clone(),
                namespace: String::new(),
                display: file.replace(".x", ""),
                kind: SymbolKind::Function,
                visibility: Visibility::Public,
                lang: LangId::from("rust"),
                file: file.clone(),
                span: Span {
                    line: 1,
                    col: 1,
                    end_line: 2,
                    end_col: 1,
                },
                body: None,
                parameters: 0,
            })
            .collect(),
    }
}

/// What a test check does when it runs.
type Body = fn(&CheckCtx, &Facts, &mut dyn EventSink) -> Result<CheckResult>;

struct TestCheck {
    manifest: Manifest,
    body: Body,
}

impl Check for TestCheck {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, _: &CheckCtx, scope: &Scope) -> Result<Plan> {
        let targets = scope.symbols.as_ref().map_or(0, |keys| keys.len() as u64);
        Ok(Plan::estimate(self.manifest.cost, targets))
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        (self.body)(ctx, facts, sink)
    }
}

fn manifest(id: &str, needs: &[FactKind]) -> Manifest {
    Manifest {
        id: CheckId::from(id),
        version: 1,
        tier: Tier::try_from(0).unwrap(),
        cost: CostClass::Quick,
        needs: needs.to_vec(),
        wants: vec![],
        findings: vec![],
        metrics: vec![],
        stability: Stability::Experimental,
        summary: String::new(),
    }
}

fn passing(id: &str, score: Option<Score>) -> CheckResult {
    CheckResult {
        id: CheckId::from(id),
        status: CheckStatus::Pass,
        tier: Tier::try_from(0).unwrap(),
        cost: CostClass::Quick,
        score,
        delta: None,
        findings: 0,
        duration_ms: 0,
        skipped: None,
        oracle: None,
    }
}

fn check(id: &str, needs: &[FactKind], body: Body) -> Box<dyn Check> {
    Box::new(TestCheck {
        manifest: manifest(id, needs),
        body,
    })
}

/// A world: registries, a repository directory, and its configuration.
struct World {
    dir: tempfile::TempDir,
    checks: CheckRegistry,
    producers: ProducerRegistry,
    symbols_made: Arc<AtomicUsize>,
    config: Config,
}

impl World {
    /// `Symbols` for rust and go, `Decisions` for rust only; `cupel.toml` holds `toml`.
    fn new(checks: Vec<Box<dyn Check>>, toml: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("cupel.toml"), toml).unwrap();
        let symbols_made = Arc::new(AtomicUsize::new(0));
        let mut producers = ProducerRegistry::default();
        producers.register::<Symbols, _>(Counting {
            langs: vec!["rust", "go"],
            count: Arc::clone(&symbols_made),
            make: symbols_of,
        });
        producers.register::<Decisions, _>(Counting {
            langs: vec!["rust"],
            count: Arc::new(AtomicUsize::new(0)),
            make: |_| Decisions::default(),
        });
        let mut registry = CheckRegistry::default();
        for check in checks {
            registry.register(check).unwrap();
        }
        let config = Config::load(&ConfigRequest {
            root: dir.path().to_path_buf(),
            ..ConfigRequest::default()
        })
        .unwrap();
        Self {
            dir,
            checks: registry,
            producers,
            symbols_made,
            config,
        }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn run(
        &self,
        langs: &[&str],
        setup: impl FnOnce(&mut Job<'_>),
    ) -> (Result<Outcome>, Vec<Event>) {
        self.execute(langs, setup, false)
    }

    fn plan(&self, langs: &[&str]) -> Outcome {
        self.execute(langs, |_| {}, true).0.unwrap()
    }

    fn execute(
        &self,
        langs: &[&str],
        setup: impl FnOnce(&mut Job<'_>),
        dry_run: bool,
    ) -> (Result<Outcome>, Vec<Event>) {
        let writer = Writer::new(self.root(), false).unwrap();
        let mut job = Job {
            run: RunId::generate(),
            writer: &writer,
            cancel: Cancellation::new(),
            config: &self.config,
            selected: self.checks.iter().collect(),
            langs: langs.iter().map(|id| lang(id)).collect(),
            files: vec!["a.x".to_owned(), "b.x".to_owned()],
            selectors: Vec::new(),
            selectors_must_match: true,
        };
        setup(&mut job);
        let scheduler = Scheduler {
            checks: &self.checks,
            producers: &self.producers,
        };
        let mut events = Vec::new();
        let outcome = if dry_run {
            scheduler.plan(&job, &mut events)
        } else {
            scheduler.run(&job, &mut events)
        };
        (outcome, events)
    }
}

fn ok(_: &CheckCtx, _: &Facts, _: &mut dyn EventSink) -> Result<CheckResult> {
    Ok(passing("t.ok", None))
}

// ---------------------------------------------------------------- the fact DAG

#[test]
fn symbols_are_produced_once_when_two_checks_need_them() {
    let world = World::new(
        vec![
            check("t.one", &[FactKind::Symbols], ok),
            check("t.two", &[FactKind::Symbols, FactKind::Decisions], ok),
        ],
        "",
    );
    let (outcome, _) = world.run(&["rust"], |_| {});
    let outcome = outcome.unwrap();
    assert_eq!(world.symbols_made.load(Ordering::SeqCst), 1);
    assert_eq!(outcome.checks.len(), 2);
    assert!(outcome.checks.iter().all(|c| c.status == CheckStatus::Pass));
}

#[test]
fn each_check_is_served_exactly_its_declared_facts() {
    fn symbols_only(_: &CheckCtx, facts: &Facts, _: &mut dyn EventSink) -> Result<CheckResult> {
        assert_eq!(facts.get::<Symbols>()?.symbols.len(), 2);
        assert_eq!(
            facts.get::<Decisions>().unwrap_err(),
            Error::UndeclaredFact {
                kind: FactKind::Decisions
            }
        );
        Ok(passing("t.symbols", None))
    }
    let world = World::new(
        vec![check("t.symbols", &[FactKind::Symbols], symbols_only)],
        "",
    );
    let (outcome, _) = world.run(&["rust"], |_| {});
    assert_eq!(outcome.unwrap().checks[0].status, CheckStatus::Pass);
}

#[test]
fn unsupported_languages_are_skipped_with_the_missing_fact() {
    let world = World::new(
        vec![check(
            "t.deep",
            &[FactKind::Symbols, FactKind::Decisions],
            ok,
        )],
        "",
    );
    let (outcome, events) = world.run(&["go", "rust"], |_| {});
    let checks = outcome.unwrap().checks;
    assert_eq!(checks.len(), 2, "{checks:?}");
    assert_eq!(checks[0].status, CheckStatus::Pass);
    assert_eq!(checks[1].status, CheckStatus::Skipped);
    let skipped = checks[1].skipped.as_ref().unwrap();
    assert_eq!(skipped.reason, SkipReason::Unsupported);
    assert_eq!(skipped.fact, Some(FactKind::Decisions));
    assert_eq!(skipped.lang, Some(LangId::from("go")));
    assert_eq!(skipped.see.as_deref(), Some("cupel describe matrix"));
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Skip {
            reason: SkipReason::Unsupported,
            ..
        }
    )));
}

#[test]
fn a_check_no_language_supports_has_only_skipped_entries() {
    let world = World::new(vec![check("t.deep", &[FactKind::Decisions], ok)], "");
    let (outcome, _) = world.run(&["go"], |_| {});
    let checks = outcome.unwrap().checks;
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].status, CheckStatus::Skipped);
}

#[test]
fn producer_findings_are_kept_once_by_id() {
    let world = World::new(
        vec![check(
            "t.two",
            &[FactKind::Symbols, FactKind::Decisions],
            ok,
        )],
        "",
    );
    let (outcome, events) = world.run(&["rust", "go"], |_| {});
    let parse_errors = outcome
        .unwrap()
        .findings
        .iter()
        .filter(|finding| finding.kind == "lang.parse-error")
        .count();
    assert_eq!(parse_errors, 1);
    let streamed = events
        .iter()
        .filter(|event| matches!(event, Event::Finding(_)))
        .count();
    assert_eq!(streamed, 1);
}

// ---------------------------------------------------------------- results and events

#[test]
fn per_language_results_combine_into_one_entry_per_check() {
    fn scored(ctx: &CheckCtx, _: &Facts, sink: &mut dyn EventSink) -> Result<CheckResult> {
        let n = ctx
            .scope
            .symbols
            .as_ref()
            .map_or(0, |keys| keys.len() as u64);
        if ctx.config.contains_key("warn") {
            sink.emit(Event::Finding(Box::new(finding(
                "t.scored",
                "t.flagged",
                "a.x",
                Severity::Warn,
                0,
            ))));
        }
        let value = ctx
            .config
            .get("value")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        Ok(passing(
            "t.scored",
            Some(Score {
                value,
                unit: "ratio".to_owned(),
                n,
            }),
        ))
    }
    let world = World::new(
        vec![check("t.scored", &[FactKind::Symbols], scored)],
        "[checks.\"t.scored\"]\nvalue = 1.0\n[checks.\"t.scored\".lang.go]\nvalue = 0.0\nwarn = true\n",
    );
    let (outcome, _) = world.run(&["go", "rust"], |_| {});
    let checks = outcome.unwrap().checks;
    assert_eq!(checks.len(), 1);
    let score = checks[0].score.as_ref().unwrap();
    assert!((score.value - 0.5).abs() < 1e-9, "{score:?}");
    assert_eq!(score.n, 4);
    assert_eq!(checks[0].findings, 1);
    assert_eq!(
        checks[0].status,
        CheckStatus::Fail,
        "a warn finding fails the check"
    );
}

#[test]
fn events_follow_the_plan_skip_start_finding_progress_end_order() {
    fn flagging(_: &CheckCtx, _: &Facts, sink: &mut dyn EventSink) -> Result<CheckResult> {
        sink.emit(Event::Finding(Box::new(finding(
            "t.flag",
            "t.flagged",
            "a.x",
            Severity::Warn,
            0,
        ))));
        Ok(passing("t.flag", None))
    }
    let world = World::new(
        vec![check(
            "t.flag",
            &[FactKind::Symbols, FactKind::Decisions],
            flagging,
        )],
        "",
    );
    let (_, events) = world.run(&["go", "rust"], |_| {});
    let names: Vec<&str> = events
        .iter()
        .map(|event| match event {
            Event::Plan { .. } => "plan",
            Event::Skip { .. } => "skip",
            Event::CheckStart { .. } => "check.start",
            Event::Finding(finding) if finding.check.as_str() == "lang" => "producer-finding",
            Event::Finding(_) => "finding",
            Event::Progress { .. } => "progress",
            Event::CheckEnd { .. } => "check.end",
            Event::RunEnd { .. } => "run.end",
        })
        .collect();
    assert_eq!(
        names,
        [
            "producer-finding",
            "plan",
            "skip",
            "check.start",
            "finding",
            "progress",
            "check.end"
        ]
    );
}

#[test]
fn no_language_in_scope_is_a_pass_with_no_score() {
    let world = World::new(vec![check("t.one", &[FactKind::Symbols], ok)], "");
    let (outcome, _) = world.run(&[], |job| job.files.clear());
    let outcome = outcome.unwrap();
    assert_eq!(outcome.symbols, 0);
    assert_eq!(outcome.checks.len(), 1);
    assert_eq!(outcome.checks[0].status, CheckStatus::Pass);
    assert_eq!(outcome.checks[0].score, None);
}

// ---------------------------------------------------------------- scope and cancellation

#[test]
fn symbol_selectors_limit_what_each_check_plans_and_sees() {
    fn counts(ctx: &CheckCtx, _: &Facts, _: &mut dyn EventSink) -> Result<CheckResult> {
        let n = ctx
            .scope
            .symbols
            .as_ref()
            .map_or(0, |keys| keys.len() as u64);
        Ok(passing(
            "t.count",
            Some(Score {
                value: 0.0,
                unit: "u".to_owned(),
                n,
            }),
        ))
    }
    let world = World::new(vec![check("t.count", &[FactKind::Symbols], counts)], "");
    let (outcome, _) = world.run(&["rust"], |job| {
        job.selectors = vec![SymbolSelector::parse("b").unwrap()];
    });
    let outcome = outcome.unwrap();
    assert_eq!(outcome.symbols, 1);
    assert_eq!(outcome.checks[0].score.as_ref().unwrap().n, 1);

    let (unmatched, _) = world.run(&["rust"], |job| {
        job.selectors = vec![SymbolSelector::parse("nope").unwrap()];
    });
    let Err(Error::Argument(error)) = unmatched else {
        panic!("expected a usage error")
    };
    assert_eq!(error.argument, "--symbols");

    let (in_range, _) = world.run(&["rust"], |job| {
        job.selectors = vec![SymbolSelector::parse("nope").unwrap()];
        job.selectors_must_match = false;
    });
    assert_eq!(in_range.unwrap().symbols, 0);
}

#[test]
fn a_cancelled_run_skips_unstarted_work_with_reason_timeout() {
    let world = World::new(vec![check("t.one", &[FactKind::Symbols], ok)], "");
    let (outcome, _) = world.run(&["rust", "go"], |job| job.cancel.cancel());
    let outcome = outcome.unwrap();
    assert!(outcome.interrupted);
    assert_eq!(world.symbols_made.load(Ordering::SeqCst), 0);
    assert_eq!(outcome.checks.len(), 2);
    for (result, lang) in outcome.checks.iter().zip(["rust", "go"]) {
        let skipped = result.skipped.as_ref().unwrap();
        assert_eq!(skipped.reason, SkipReason::Timeout);
        assert_eq!(skipped.lang, Some(LangId::from(lang)));
    }
}

// ---------------------------------------------------------------- planning

#[test]
fn a_plan_lists_targets_estimates_needs_and_skips_without_running_checks() {
    fn never(_: &CheckCtx, _: &Facts, _: &mut dyn EventSink) -> Result<CheckResult> {
        unreachable!("a dry run runs no check")
    }
    let world = World::new(
        vec![
            check("t.one", &[FactKind::Symbols], never),
            check("t.deep", &[FactKind::Symbols, FactKind::Decisions], never),
        ],
        "",
    );
    let outcome = world.plan(&["go", "rust"]);
    let plan = outcome.plan.unwrap();
    assert!(outcome.checks.is_empty());
    assert_eq!(plan.would_run.len(), 2);
    assert_eq!(plan.would_run[0].targets, 4, "two symbols in each language");
    assert_eq!(plan.would_run[0].est_ms, 4);
    assert_eq!(plan.would_run[1].needs, ["symbols", "decisions"]);
    assert_eq!(plan.would_skip.len(), 1);
    assert_eq!(plan.would_skip[0].id, CheckId::from("t.deep"));
    assert_eq!(plan.would_skip[0].skipped.fact, Some(FactKind::Decisions));
}

#[test]
fn next_actions_rank_by_severity_then_tier() {
    let findings = vec![
        finding("c", "info", "a", Severity::Info, 6),
        finding("c", "warn-low", "b", Severity::Warn, 0),
        finding("c", "warn-high", "c", Severity::Warn, 3),
        finding("c", "fail", "d", Severity::Fail, 0),
    ];
    let actions = next_actions(&findings);
    let order: Vec<&str> = actions
        .iter()
        .map(|action| {
            let id = action.finding.as_ref().unwrap();
            findings.iter().find(|f| f.id == *id).unwrap().kind.as_str()
        })
        .collect();
    assert_eq!(order, ["fail", "warn-high", "warn-low", "info"]);
    assert_eq!(actions[0].priority, 1);
    let many: Vec<Finding> = (0..15)
        .map(|n| finding("c", "k", &format!("f{n}"), Severity::Warn, 0))
        .collect();
    assert_eq!(next_actions(&many).len(), 10);
}
