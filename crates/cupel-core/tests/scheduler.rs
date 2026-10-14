// ABOUTME: Scheduler v1 over synthetic registries: facts produced once and served only as declared,
// ABOUTME: unsupported pairs skipped, findings de-duplicated, cancellation, combining, planning (ADR-041).
#![allow(
    clippy::unnecessary_wraps,
    reason = "test checks share the `Check::run` signature, which returns a Result"
)]
mod support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use cupel_core::config::Config;
use cupel_core::git::{Change, ChangeKind};
use cupel_core::syntax::{Decisions, Span, SymbolDef, SymbolKind, Symbols, Visibility};
use cupel_core::{
    Action, ActionKind, Availability, BaseTree, Check, CheckCtx, CheckId, CheckRegistry,
    CheckResult, CheckStatus, CostClass, Ctx, Error, Event, EventSink, Evidence, Fact, FactKind,
    Facts, Finding, FindingId, Job, LangId, Langs, Location, Manifest, Measurement, Outcome, Plan,
    ProduceCtx, Producer, ProducerId, ProducerRegistry, Range, Result, Scheduler, Scope, Score,
    Severity, SkipReason, Skipped, SymbolSelector, Target, Tier, Writer,
};

// ---------------------------------------------------------------- fixtures

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
    spans_family: bool,
    /// Combine `scope` metrics by the mean weighted by each language's `score.n`.
    weighted: bool,
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

    fn spans_family(&self) -> bool {
        self.spans_family
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "test counts are tiny, where f64 is exact"
    )]
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        if !self.weighted {
            return results
                .iter()
                .map(|result| result.metrics[name])
                .reduce(f64::max);
        }
        let n = |result: &CheckResult| result.score.as_ref().map_or(0, |score| score.n) as f64;
        let total: f64 = results.iter().map(|result| n(result)).sum();
        let weighted: f64 = results
            .iter()
            .map(|result| result.metrics[name] * n(result))
            .sum();
        Some(weighted / total)
    }
}
fn passing(id: &str, score: Option<Score>) -> CheckResult {
    CheckResult {
        id: CheckId::from(id),
        status: CheckStatus::Pass,
        tier: Tier::try_from(0).unwrap(),
        cost: CostClass::Quick,
        score,
        metrics: BTreeMap::new(),
        delta: None,
        findings: 0,
        duration_ms: 0,
        skipped: None,
        oracle: None,
        notes: Vec::new(),
    }
}

fn check(id: &str, needs: &[FactKind], body: Body) -> Box<dyn Check> {
    Box::new(TestCheck {
        manifest: support::manifest(id, needs, &[]),
        body,
        spans_family: false,
        weighted: false,
    })
}

/// A check that runs once per language family, over every member language's facts.
fn family_check(id: &str, needs: &[FactKind], body: Body) -> Box<dyn Check> {
    Box::new(TestCheck {
        manifest: support::manifest(id, needs, &[]),
        body,
        spans_family: true,
        weighted: false,
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
        let config = support::config_at(dir.path(), toml);
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
        let mut job = support::job(&writer, &self.config, &self.checks, langs);
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

/// A check that also reads `Symbols` from a range's base side (ADR-056).
struct BaseCheck {
    manifest: Manifest,
    body: Body,
}

impl Check for BaseCheck {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, _: &CheckCtx, _: &Scope) -> Result<Plan> {
        Ok(Plan::estimate(self.manifest.cost, 1))
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

    fn base_needs(&self) -> &[FactKind] {
        &[FactKind::Symbols]
    }
}

fn base_check(id: &str, body: Body) -> Box<dyn Check> {
    Box::new(BaseCheck {
        manifest: support::manifest(id, &[FactKind::Symbols], &[]),
        body,
    })
}

/// A base side holding `gone.x`, which the range deleted, and `a.x`, which it modified.
fn base_tree(root: &Path) -> BaseTree {
    BaseTree {
        range: Range {
            commit: "0123abc".to_owned(),
            changes: vec![
                Change {
                    path: "a.x".to_owned(),
                    kind: ChangeKind::Modified,
                },
                Change {
                    path: "gone.x".to_owned(),
                    kind: ChangeKind::Deleted,
                },
                Change {
                    path: "notes.txt".to_owned(),
                    kind: ChangeKind::Added,
                },
            ],
        },
        root: root.join("base"),
        files: vec!["a.x".to_owned(), "gone.x".to_owned()],
    }
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
    assert_eq!(checks.len(), 1, "one entry per check (ADR-064): {checks:?}");
    assert_eq!(checks[0].status, CheckStatus::Pass);
    assert_eq!(checks[0].skipped, None);
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Skip {
            reason: SkipReason::Unsupported,
            ..
        }
    )));
}

#[test]
fn a_check_no_language_supports_has_one_skipped_entry_with_the_missing_fact() {
    let world = World::new(vec![check("t.deep", &[FactKind::Decisions], ok)], "");
    let (outcome, _) = world.run(&["go"], |_| {});
    let checks = outcome.unwrap().checks;
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].status, CheckStatus::Skipped);
    assert_eq!(
        checks[0].skipped,
        Some(Skipped {
            reason: SkipReason::Unsupported,
            fact: Some(FactKind::Decisions),
            lang: Some(LangId::from("go")),
            tool: None,
            see: Some("cupel describe matrix".to_owned()),
        })
    );
}

#[test]
fn a_check_every_language_skips_alike_has_one_entry_naming_no_language() {
    fn unconfigured(ctx: &CheckCtx, _: &Facts, _: &mut dyn EventSink) -> Result<CheckResult> {
        let lang = ctx.config["language"].as_str().unwrap();
        Ok(CheckResult {
            status: CheckStatus::Skipped,
            skipped: Some(Skipped {
                reason: SkipReason::Unconfigured,
                fact: Some(FactKind::CoverageMap),
                lang: Some(LangId::from(lang)),
                tool: None,
                see: Some("cupel describe t.unconfigured".to_owned()),
            }),
            ..passing("t.unconfigured", None)
        })
    }
    let world = World::new(
        vec![check("t.unconfigured", &[FactKind::Symbols], unconfigured)],
        "[checks.\"t.unconfigured\".lang.go]\nlanguage = \"go\"\n\
         [checks.\"t.unconfigured\".lang.rust]\nlanguage = \"rust\"\n",
    );
    let (outcome, _) = world.run(&["go", "rust"], |_| {});
    let checks = outcome.unwrap().checks;
    assert_eq!(checks.len(), 1, "{checks:?}");
    assert_eq!(checks[0].status, CheckStatus::Skipped);
    assert_eq!(
        checks[0].skipped,
        Some(Skipped {
            reason: SkipReason::Unconfigured,
            fact: Some(FactKind::CoverageMap),
            lang: None,
            tool: None,
            see: Some("cupel describe t.unconfigured".to_owned()),
        })
    );
}

#[test]
fn languages_skipped_for_different_reasons_keep_the_first_languages_reason() {
    let world = World::new(
        vec![check(
            "t.deep",
            &[FactKind::Symbols, FactKind::Decisions],
            ok,
        )],
        "",
    );
    let (outcome, _) = world.run(&["go", "rust"], |job| {
        job.cancel.cancel();
    });
    let checks = outcome.unwrap().checks;
    assert_eq!(checks.len(), 1, "{checks:?}");
    let skipped = checks[0].skipped.as_ref().unwrap();
    assert_eq!(
        (skipped.reason, skipped.lang.as_ref()),
        (SkipReason::Unsupported, Some(&LangId::from("go")))
    );
}

#[test]
fn a_language_cut_short_makes_the_combined_entry_partial_with_its_skip() {
    fn cancelling(ctx: &CheckCtx, _: &Facts, _: &mut dyn EventSink) -> Result<CheckResult> {
        ctx.cancel.cancel();
        Ok(passing("t.one", None))
    }
    let world = World::new(vec![check("t.one", &[FactKind::Symbols], cancelling)], "");
    let (outcome, _) = world.run(&["rust", "go"], |_| {});
    let outcome = outcome.unwrap();
    assert!(outcome.interrupted);
    assert_eq!(outcome.checks.len(), 1, "{:?}", outcome.checks);
    assert_eq!(outcome.checks[0].status, CheckStatus::Partial);
    let skipped = outcome.checks[0].skipped.as_ref().unwrap();
    assert_eq!(
        (skipped.reason, skipped.lang.as_ref()),
        (SkipReason::Timeout, Some(&LangId::from("go")))
    );
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

// ---------------------------------------------------------------- family-wide checks

/// Reports, as its score, how many languages' `Symbols` it saw (`value`) and how many symbols
/// are in its scope (`n`).
fn family_symbols(ctx: &CheckCtx, facts: &Facts, _: &mut dyn EventSink) -> Result<CheckResult> {
    let seen = facts.family::<Symbols>()?;
    let n = ctx
        .scope
        .symbols
        .as_ref()
        .map_or(0, |keys| keys.len() as u64);
    Ok(passing(
        "t.family",
        Some(Score {
            value: f64::from(u32::try_from(seen.len()).unwrap()),
            unit: "languages".to_owned(),
            n,
        }),
    ))
}

#[test]
fn a_family_wide_check_runs_once_per_family_with_every_member_languages_facts() {
    let world = World::new(
        vec![family_check(
            "t.family",
            &[FactKind::Symbols],
            family_symbols,
        )],
        "",
    );
    let (outcome, events) = world.run(&["go", "rust"], |job| {
        job.langs = vec![support::lang("go", "pair"), support::lang("rust", "pair")];
    });
    let outcome = outcome.unwrap();
    assert_eq!(outcome.checks.len(), 1, "{:?}", outcome.checks);
    let score = outcome.checks[0].score.as_ref().unwrap();
    assert!((score.value - 2.0).abs() < f64::EPSILON, "{score:?}");
    let progress = events
        .iter()
        .filter(|event| matches!(event, Event::Progress { .. }))
        .count();
    assert_eq!(progress, 1, "one task for the family");
}

#[test]
fn languages_of_different_families_run_as_separate_tasks() {
    let world = World::new(
        vec![family_check(
            "t.family",
            &[FactKind::Symbols],
            family_symbols,
        )],
        "",
    );
    let (outcome, events) = world.run(&["go", "rust"], |_| {});
    let outcome = outcome.unwrap();
    let score = outcome.checks[0].score.as_ref().unwrap();
    assert!(
        (score.value - 1.0).abs() < f64::EPSILON,
        "mean of one language each"
    );
    let progress = events
        .iter()
        .filter(|event| matches!(event, Event::Progress { .. }))
        .count();
    assert_eq!(progress, 2, "one task per family");
}

#[test]
fn an_unsupported_family_member_is_skipped_and_the_rest_of_the_family_runs() {
    fn decisions_of_family(
        _: &CheckCtx,
        facts: &Facts,
        _: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        let seen = facts.family::<Decisions>()?;
        Ok(passing(
            "t.family",
            Some(Score {
                value: f64::from(u32::try_from(seen.len()).unwrap()),
                unit: "languages".to_owned(),
                n: 1,
            }),
        ))
    }
    let world = World::new(
        vec![family_check(
            "t.family",
            &[FactKind::Symbols, FactKind::Decisions],
            decisions_of_family,
        )],
        "",
    );
    let (outcome, _) = world.run(&["go", "rust"], |job| {
        job.langs = vec![support::lang("go", "pair"), support::lang("rust", "pair")];
    });
    let checks = outcome.unwrap().checks;
    assert_eq!(checks.len(), 1, "{checks:?}");
    assert!((checks[0].score.as_ref().unwrap().value - 1.0).abs() < f64::EPSILON);
    assert_eq!(checks[0].skipped, None);
}

#[test]
fn facts_of_other_family_members_are_served_only_when_declared() {
    let mut facts = Facts::for_manifest(&support::manifest("t.family", &[FactKind::Symbols], &[]));
    facts.insert(Symbols::default()).unwrap();
    facts.insert_sibling(Symbols::default()).unwrap();
    assert_eq!(facts.family::<Symbols>().unwrap().len(), 2);
    assert_eq!(
        facts.insert_sibling(Decisions::default()).unwrap_err(),
        Error::UndeclaredFact {
            kind: FactKind::Decisions
        }
    );
    assert_eq!(
        facts.family::<Decisions>().unwrap_err(),
        Error::UndeclaredFact {
            kind: FactKind::Decisions
        }
    );
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

/// Reports `bound` (from config) as a `scope` metric and one `size` measurement per symbol.
fn measuring(ctx: &CheckCtx, _: &Facts, sink: &mut dyn EventSink) -> Result<CheckResult> {
    let bound = ctx
        .config
        .get("bound")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0);
    for (n, key) in ctx.scope.symbols.iter().flatten().enumerate() {
        sink.measure(Measurement {
            metric: "size".to_owned(),
            value: f64::from(u8::try_from(n).unwrap()),
            target: Target {
                display: format!("{key:?}"),
                symbol: None,
                location: None,
            },
        });
    }
    let mut result = passing("t.measured", None);
    result.metrics.insert("bound".to_owned(), bound);
    Ok(result)
}

#[test]
fn scope_metrics_combine_across_languages_by_the_largest_value() {
    let world = World::new(
        vec![check("t.measured", &[FactKind::Symbols], measuring)],
        "[checks.\"t.measured\"]\nbound = 3.0\n[checks.\"t.measured\".lang.go]\nbound = 7.0\n",
    );
    let (outcome, _) = world.run(&["go", "rust"], |_| {});
    let checks = outcome.unwrap().checks;
    assert_eq!(checks.len(), 1);
    assert_eq!(
        checks[0].metrics,
        BTreeMap::from([("bound".to_owned(), 7.0)])
    );
}

/// Notes what configuration names under `note`, as a check notes what it could not measure.
fn noting(ctx: &CheckCtx, _: &Facts, _: &mut dyn EventSink) -> Result<CheckResult> {
    let mut result = passing("t.noted", None);
    result.notes.extend(
        ctx.config
            .get("note")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
    );
    Ok(result)
}

#[test]
fn each_languages_notes_reach_the_combined_entry_naming_their_language() {
    let world = World::new(
        vec![check("t.noted", &[FactKind::Symbols], noting)],
        "[checks.\"t.noted\".lang.go]\nnote = \"branches: unavailable\"\n\
         [checks.\"t.noted\".lang.rust]\nnote = \"branches: unavailable\"\n",
    );
    let (outcome, _) = world.run(&["go", "rust"], |_| {});
    let checks = outcome.unwrap().checks;
    assert_eq!(
        checks[0].notes,
        ["branches: unavailable (go)", "branches: unavailable (rust)"]
    );
    let quiet = World::new(vec![check("t.noted", &[FactKind::Symbols], noting)], "");
    let (outcome, _) = quiet.run(&["go"], |_| {});
    assert!(outcome.unwrap().checks[0].notes.is_empty());
}

/// Reports `bound` as a `scope` metric over a score covering `n` targets, both from config.
fn measuring_over_n(ctx: &CheckCtx, _: &Facts, _: &mut dyn EventSink) -> Result<CheckResult> {
    let setting = |key| ctx.config.get(key).cloned().unwrap_or_default();
    let mut result = passing(
        "t.weighted",
        Some(Score {
            value: 0.0,
            unit: "n".to_owned(),
            n: setting("n").as_u64().unwrap_or(0),
        }),
    );
    result
        .metrics
        .insert("bound".to_owned(), setting("bound").as_f64().unwrap_or(0.0));
    Ok(result)
}

#[test]
fn a_check_combines_its_metrics_from_each_languages_whole_result() {
    let world = World::new(
        vec![Box::new(TestCheck {
            manifest: support::manifest("t.weighted", &[FactKind::Symbols], &[]),
            body: measuring_over_n,
            spans_family: false,
            weighted: true,
        })],
        "[checks.\"t.weighted\"]\nbound = 3.0\nn = 1\n\
         [checks.\"t.weighted\".lang.go]\nbound = 7.0\nn = 3\n",
    );
    let (outcome, _) = world.run(&["go", "rust"], |_| {});
    let checks = outcome.unwrap().checks;
    assert_eq!(
        checks[0].metrics,
        BTreeMap::from([("bound".to_owned(), 6.0)]),
        "(7 × 3 + 3 × 1) / 4"
    );
}

#[test]
fn measurements_are_kept_with_their_check_and_language_and_never_streamed() {
    let world = World::new(
        vec![check("t.measured", &[FactKind::Symbols], measuring)],
        "",
    );
    let (outcome, events) = world.run(&["go", "rust"], |_| {});
    let measured = outcome.unwrap().measurements;
    let langs: Vec<String> = measured.iter().map(|m| m.lang.to_string()).collect();
    assert_eq!(langs, ["go", "go", "rust", "rust"]);
    assert!(
        measured
            .iter()
            .all(|m| m.check == CheckId::from("t.measured"))
    );
    assert!(measured.iter().all(|m| m.measurement.metric == "size"));
    assert_eq!(
        events.len(),
        6,
        "a parse finding, plan, start, two progress, end: {events:?}"
    );
}

/// An event's tag, telling a producer's finding (from check `lang`) from a check's.
fn event_name(event: &Event) -> &'static str {
    match event {
        Event::Plan { .. } => "plan",
        Event::Skip { .. } => "skip",
        Event::CheckStart { .. } => "check.start",
        Event::Finding(finding) if finding.check.as_str() == "lang" => "producer-finding",
        Event::Finding(_) => "finding",
        Event::Progress { .. } => "progress",
        Event::CheckEnd { .. } => "check.end",
        Event::RunEnd { .. } => "run.end",
    }
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
    let names: Vec<&str> = events.iter().map(event_name).collect();
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
    assert_eq!(outcome.checks.len(), 1);
    let skipped = outcome.checks[0].skipped.as_ref().unwrap();
    assert_eq!(skipped.reason, SkipReason::Timeout);
    assert_eq!(skipped.lang, None, "both languages were cut short");
}

#[test]
fn a_check_that_skips_itself_is_reported_without_interrupting_the_run() {
    fn unconfigured(_: &CheckCtx, _: &Facts, _: &mut dyn EventSink) -> Result<CheckResult> {
        Ok(CheckResult {
            status: CheckStatus::Skipped,
            skipped: Some(Skipped {
                reason: SkipReason::Unconfigured,
                fact: Some(FactKind::CoverageMap),
                lang: Some(LangId::from("rust")),
                tool: None,
                see: None,
            }),
            ..passing("t.unconfigured", None)
        })
    }
    let world = World::new(
        vec![check("t.unconfigured", &[FactKind::Symbols], unconfigured)],
        "",
    );
    let (outcome, _) = world.run(&["rust"], |_| {});
    let outcome = outcome.unwrap();
    assert!(!outcome.interrupted, "only a timeout interrupts a run");
    assert_eq!(outcome.checks.len(), 1);
    assert_eq!(outcome.checks[0].status, CheckStatus::Skipped);
    let skipped = outcome.checks[0].skipped.as_ref().unwrap();
    assert_eq!(skipped.reason, SkipReason::Unconfigured);
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
fn a_check_that_reads_the_base_is_skipped_once_without_a_range() {
    let world = World::new(vec![base_check("t.base", ok)], "");
    let (outcome, events) = world.run(&["go", "rust"], |_| {});
    let checks = outcome.unwrap().checks;
    assert_eq!(checks.len(), 1, "{checks:?}");
    assert_eq!(checks[0].status, CheckStatus::Skipped);
    assert_eq!(
        checks[0].skipped,
        Some(Skipped {
            reason: SkipReason::NeedsRange,
            fact: None,
            lang: None,
            tool: None,
            see: Some("cupel check --changed --only t.base".to_owned()),
        })
    );
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Skip {
            reason: SkipReason::NeedsRange,
            ..
        }
    )));
    assert_eq!(world.symbols_made.load(Ordering::SeqCst), 2, "scope only");
}

#[test]
fn under_a_range_a_check_reads_both_sides_and_the_range() {
    fn both_sides(_: &CheckCtx, facts: &Facts, _: &mut dyn EventSink) -> Result<CheckResult> {
        let files = |symbols: &Symbols| -> Vec<String> {
            symbols.symbols.iter().map(|s| s.file.clone()).collect()
        };
        assert_eq!(files(facts.get::<Symbols>()?), ["a.x", "b.x"]);
        assert_eq!(files(facts.base::<Symbols>()?), ["a.x", "gone.x"]);
        assert_eq!(
            facts.base::<Decisions>().unwrap_err(),
            Error::UndeclaredFact {
                kind: FactKind::Decisions
            }
        );
        let range = facts.range().expect("a range");
        assert_eq!(range.commit, "0123abc");
        assert_eq!(range.changes.len(), 3);
        Ok(passing("t.base", None))
    }
    fn head_only(_: &CheckCtx, facts: &Facts, _: &mut dyn EventSink) -> Result<CheckResult> {
        assert_eq!(
            facts.base::<Symbols>().unwrap_err(),
            Error::AbsentFact {
                kind: FactKind::Symbols
            }
        );
        assert_eq!(facts.range().map(|r| r.commit.as_str()), Some("0123abc"));
        Ok(passing("t.head", None))
    }
    let world = World::new(
        vec![
            base_check("t.base", both_sides),
            check("t.head", &[FactKind::Symbols], head_only),
        ],
        "",
    );
    let root = world.root().to_path_buf();
    let (outcome, _) = world.run(&["rust"], |job| job.base = Some(base_tree(&root)));
    let checks = outcome.unwrap().checks;
    assert_eq!(checks.len(), 2, "{checks:?}");
    assert!(
        checks.iter().all(|c| c.status == CheckStatus::Pass),
        "{checks:?}"
    );
    assert_eq!(
        world.symbols_made.load(Ordering::SeqCst),
        2,
        "once per side"
    );
}
