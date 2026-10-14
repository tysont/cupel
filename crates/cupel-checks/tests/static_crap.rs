// ABOUTME: static.crap over synthetic facts: CRAP from cyclomatic complexity with coverage counted as zero,
// ABOUTME: its bands, the skip without `no_coverage`, test-file hints, settings, scope, and the score (ADR-048).
use std::collections::BTreeSet;

use cupel_checks::register;
use cupel_core::config::ConfigError;
use cupel_core::syntax::{
    Decision, DecisionKind, Decisions, NormNode, NormTree, NormalizedSymbol, Span, SymbolDecisions,
    SymbolDef, SymbolKind, Symbols, TestConventions, TestPairing, Visibility,
};
use cupel_core::{
    ActionKind, Cancellation, Check, CheckCtx, CheckId, CheckRegistry, CheckResult, CheckStatus,
    Error, Event, FactKind, Facts, Finding, FindingId, LangId, Plan, Scope, Score, Severity,
    SkipReason, SymbolHash, Writer,
};
use serde_json::{Map, Value, json};

const ID: &str = "static.crap";

/// A function `name` in `f.x` starting at `line`.
fn symbol(name: &str, line: u32) -> SymbolDef {
    SymbolDef {
        name: name.to_owned(),
        namespace: "m".to_owned(),
        display: format!("m::{name}"),
        kind: SymbolKind::Function,
        visibility: Visibility::Public,
        lang: LangId::from("x"),
        file: "f.x".to_owned(),
        span: Span {
            line,
            col: 1,
            end_line: line + 2,
            end_col: 2,
        },
        body: None,
        parameters: 0,
    }
}

/// Decisions giving a cyclomatic complexity of `cyclomatic`.
fn branches(cyclomatic: u32) -> Vec<Decision> {
    (1..cyclomatic)
        .map(|_| Decision {
            kind: DecisionKind::Branch,
            nesting: 0,
            cyclomatic: 1,
            cognitive: 1,
        })
        .collect()
}

fn hash_of(line: u32) -> SymbolHash {
    format!("sh_{line:016x}").parse().unwrap()
}

/// Facts for `symbols`, each with its decisions, and optionally the language's test conventions.
struct World {
    symbols: Vec<(SymbolDef, Vec<Decision>)>,
    conventions: Option<TestConventions>,
}

impl World {
    fn of(symbols: Vec<(SymbolDef, Vec<Decision>)>) -> Self {
        Self {
            symbols,
            conventions: None,
        }
    }

    fn facts(&self, check: &dyn Check) -> Facts {
        let mut facts = Facts::for_manifest(check.manifest());
        facts
            .insert(Symbols {
                symbols: self.symbols.iter().map(|(s, _)| s.clone()).collect(),
            })
            .unwrap();
        facts
            .insert(Decisions {
                symbols: self
                    .symbols
                    .iter()
                    .map(|(s, d)| SymbolDecisions {
                        symbol: s.key(),
                        decisions: d.clone(),
                    })
                    .collect(),
            })
            .unwrap();
        facts
            .insert(NormTree {
                symbols: self
                    .symbols
                    .iter()
                    .map(|(s, _)| NormalizedSymbol {
                        symbol: s.key(),
                        hash: hash_of(s.span.line),
                        root: NormNode {
                            label: "fn".to_owned(),
                            children: Vec::new(),
                        },
                    })
                    .collect(),
            })
            .unwrap();
        if let Some(conventions) = &self.conventions {
            facts.insert(conventions.clone()).unwrap();
        }
        facts
    }

    fn scope(&self) -> Scope {
        Scope {
            files: vec!["f.x".to_owned()],
            symbols: Some(self.symbols.iter().map(|(s, _)| s.key()).collect()),
        }
    }
}

fn registry() -> CheckRegistry {
    let mut registry = CheckRegistry::default();
    register(&mut registry);
    registry
}

fn ctx(scope: Scope, config: Value) -> (CheckCtx, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let writer = Writer::new(dir.path(), false).unwrap();
    let Value::Object(config) = config else {
        panic!("config is a table")
    };
    let ctx = CheckCtx {
        scope,
        config,
        writer: writer.scratch(&cupel_core::RunId::generate()),
        cancel: Cancellation::new(),
    };
    (ctx, dir)
}

/// The configuration that scores cyclomatic complexity alone, plus `extra`.
fn cc_only(extra: Value) -> Value {
    let mut config = json!({"no_coverage": true});
    if let (Value::Object(config), Value::Object(extra)) = (&mut config, extra) {
        config.extend(extra);
    }
    config
}

/// Runs the check over `world` with `config`; its result and findings.
fn run(world: &World, config: Value) -> (CheckResult, Vec<Finding>) {
    run_in(world, world.scope(), config)
}

fn run_in(world: &World, scope: Scope, config: Value) -> (CheckResult, Vec<Finding>) {
    let registry = registry();
    let check = registry.get(&CheckId::from(ID)).unwrap();
    let (ctx, _dir) = ctx(scope, config);
    let plan = check.plan(&ctx, &ctx.scope).unwrap();
    let mut events = Vec::new();
    let result = check
        .run(&ctx, &world.facts(check), &plan, &mut events)
        .unwrap();
    let findings = events
        .into_iter()
        .filter_map(|event| match event {
            Event::Finding(finding) => Some(*finding),
            _ => None,
        })
        .collect();
    (result, findings)
}

fn kinds(findings: &[Finding]) -> Vec<(String, String)> {
    findings
        .iter()
        .map(|f| (f.symbol.as_ref().unwrap().display.clone(), f.kind.clone()))
        .collect()
}

#[test]
fn the_manifest_declares_its_facts_kinds_actions_and_metrics() {
    let registry = registry();
    let manifest = registry.get(&CheckId::from(ID)).unwrap().manifest();
    assert_eq!(
        manifest.needs,
        [FactKind::Symbols, FactKind::Decisions, FactKind::NormTree]
    );
    assert_eq!(
        manifest.wants,
        [FactKind::CoverageMap, FactKind::TestConventions]
    );
    assert_eq!(manifest.tier.value(), 0);
    let kinds: Vec<(&str, ActionKind)> = manifest
        .findings
        .iter()
        .map(|spec| (spec.kind.as_str(), spec.action))
        .collect();
    assert_eq!(
        kinds,
        [
            ("crap.high", ActionKind::WriteTest),
            ("crap.watch", ActionKind::Review),
        ]
    );
    let metrics: Vec<&str> = manifest.metrics.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(metrics, ["max", "mean"]);
    for spec in &manifest.findings {
        assert_eq!(spec.evidence["type"], json!("object"), "{}", spec.kind);
    }
}

#[test]
fn without_coverage_or_no_coverage_the_check_skips_and_says_how_to_run() {
    let world = World::of(vec![(symbol("risky", 1), branches(6))]);
    for config in [json!({}), json!({"no_coverage": false})] {
        let (result, findings) = run(&world, config);
        assert!(findings.is_empty());
        assert_eq!(result.status, CheckStatus::Skipped);
        assert_eq!(result.score, None);
        let skipped = result.skipped.expect("a skipped object");
        assert_eq!(skipped.reason, SkipReason::Unconfigured);
        assert_eq!(skipped.fact, Some(FactKind::CoverageMap));
        assert_eq!(skipped.lang, Some(LangId::from("x")));
        assert_eq!(skipped.see.as_deref(), Some("cupel describe static.crap"));
    }
}

#[test]
fn crap_is_cyclomatic_squared_plus_cyclomatic_in_bands_at_their_defaults() {
    // CRAP at zero coverage: 1 → 2, 2 → 6, 4 → 20, 5 → 30, 6 → 42.
    let world = World::of(vec![
        (symbol("straight", 1), branches(1)),
        (symbol("lowest_watch", 10), branches(2)),
        (symbol("highest_watch", 20), branches(4)),
        (symbol("lowest_high", 30), branches(5)),
        (symbol("risky", 40), branches(6)),
    ]);
    let (result, findings) = run(&world, cc_only(json!({})));
    let expected: Vec<(String, String)> = [
        ("m::lowest_watch", "crap.watch"),
        ("m::highest_watch", "crap.watch"),
        ("m::lowest_high", "crap.high"),
        ("m::risky", "crap.high"),
    ]
    .iter()
    .map(|(s, k)| ((*s).to_owned(), (*k).to_owned()))
    .collect();
    assert_eq!(kinds(&findings), expected);
    let crap: Vec<Value> = findings
        .iter()
        .map(|f| f.evidence.0["crap"].clone())
        .collect();
    assert_eq!(crap, [json!(6.0), json!(20.0), json!(30.0), json!(42.0)]);
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.findings, 4);
    assert_eq!(
        result.score,
        Some(Score {
            value: 42.0,
            unit: "max".to_owned(),
            n: 5
        })
    );
}

#[test]
fn a_high_finding_is_a_warning_to_write_tests_identified_by_its_symbol_hash() {
    let world = World::of(vec![(symbol("risky", 7), branches(6))]);
    let (_, findings) = run(&world, cc_only(json!({})));
    let [finding] = findings.as_slice() else {
        panic!("one finding expected: {findings:?}")
    };
    let hash = hash_of(7);
    let check = CheckId::from(ID);
    assert_eq!(
        finding.id,
        FindingId::compute(&check, "crap.high", &hash, None)
    );
    assert_eq!(finding.severity, Severity::Warn);
    assert_eq!(finding.tier.value(), 0);
    assert_eq!(finding.action.kind, ActionKind::WriteTest);
    assert_eq!(
        finding.repro,
        format!("cupel check --only static.crap --symbols {hash}")
    );
    assert_eq!(
        finding.evidence.0,
        *json!({"crap": 42.0, "cyclomatic": 6, "coverage": null, "watch": 5, "high": 30})
            .as_object()
            .unwrap()
    );
    assert!(finding.title.contains("CRAP 42"), "{}", finding.title);
    assert!(finding.action.hints.is_empty(), "no test conventions given");
}

#[test]
fn a_watch_finding_is_information_to_review_and_alone_leaves_the_check_passing() {
    let world = World::of(vec![(symbol("branchy", 1), branches(3))]);
    let (result, findings) = run(&world, cc_only(json!({})));
    let [finding] = findings.as_slice() else {
        panic!("one finding expected: {findings:?}")
    };
    assert_eq!(finding.kind, "crap.watch");
    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(finding.action.kind, ActionKind::Review);
    assert_eq!(finding.evidence.0["crap"], json!(12.0));
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.findings, 1);
}

#[test]
fn a_high_finding_names_the_nearest_test_file_when_conventions_pair_one() {
    let mut world = World::of(vec![
        (symbol("risky", 1), branches(6)),
        (symbol("branchy", 10), branches(3)),
    ]);
    world.conventions = Some(TestConventions {
        test_globs: vec!["**/*_test.x".to_owned()],
        pairs: vec![TestPairing {
            source: "f.x".to_owned(),
            tests: vec!["f_test.x".to_owned(), "g_test.x".to_owned()],
        }],
    });
    let (_, findings) = run(&world, cc_only(json!({})));
    let high = findings.iter().find(|f| f.kind == "crap.high").unwrap();
    assert_eq!(
        Value::Object(high.action.hints.clone()),
        json!({"nearest_test_file": "f_test.x"})
    );
    let watch = findings.iter().find(|f| f.kind == "crap.watch").unwrap();
    assert!(watch.action.hints.is_empty());
}

#[test]
fn symbols_in_test_files_are_not_scored() {
    let mut test = symbol("test_risky", 10);
    test.file = "f_test.x".to_owned();
    let mut world = World::of(vec![(symbol("risky", 1), branches(6)), (test, branches(6))]);
    world.conventions = Some(TestConventions {
        test_globs: vec!["**/*_test.x".to_owned()],
        pairs: vec![],
    });
    let (result, findings) = run(&world, cc_only(json!({})));
    assert_eq!(
        kinds(&findings),
        [("m::risky".to_owned(), "crap.high".to_owned())]
    );
    assert_eq!(result.score.map(|score| score.n), Some(1));
}

#[test]
fn bands_come_from_the_configuration_the_context_carries() {
    let world = World::of(vec![(symbol("branchy", 1), branches(3))]);
    let kind = |config| {
        let (_, findings) = run(&world, cc_only(config));
        findings.first().map(|f| f.kind.clone())
    };
    assert_eq!(kind(json!({"high": 12})), Some("crap.high".to_owned()));
    assert_eq!(kind(json!({"watch": 13})), None);
    assert_eq!(kind(json!({"watch": 12})), Some("crap.watch".to_owned()));
}

#[test]
fn only_symbols_in_scope_are_scored() {
    let world = World::of(vec![
        (symbol("inside", 1), branches(6)),
        (symbol("outside", 10), branches(8)),
    ]);
    let scope = Scope {
        files: vec!["f.x".to_owned()],
        symbols: Some(BTreeSet::from([world.symbols[0].0.key()])),
    };
    let (result, findings) = run_in(&world, scope, cc_only(json!({})));
    assert_eq!(
        kinds(&findings),
        [("m::inside".to_owned(), "crap.high".to_owned())]
    );
    let worst = result.score.unwrap();
    assert_eq!((worst.value, worst.n), (42.0, 1));
}

#[test]
fn nothing_in_scope_passes_without_needing_coverage() {
    let world = World::of(vec![]);
    let (result, findings) = run(&world, json!({}));
    assert!(findings.is_empty());
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.skipped, None);
    assert_eq!(result.score.map(|score| score.n), Some(0));
}

#[test]
fn unknown_settings_and_bad_values_are_configuration_errors() {
    let registry = registry();
    let check = registry.get(&CheckId::from(ID)).unwrap();
    for (config, key) in [
        (json!({"threshold": 3}), "threshold"),
        (json!({"no_coverage": "yes"}), "no_coverage"),
        (json!({"high": -1}), "high"),
        (json!({"watch": 2.5}), "watch"),
    ] {
        let (ctx, _dir) = ctx(Scope::default(), config);
        let error = check.plan(&ctx, &Scope::default()).unwrap_err();
        let Error::Config(ConfigError::CheckSetting {
            key: named, valid, ..
        }) = error
        else {
            panic!("expected a check-setting error, got {error:?}")
        };
        assert_eq!(named, key);
        assert_eq!(valid, ["high", "watch", "no_coverage"]);
    }
}

#[test]
fn plans_count_the_symbols_in_scope_at_quick_cost() {
    let registry = registry();
    let check = registry.get(&CheckId::from(ID)).unwrap();
    let scope = Scope {
        files: vec![],
        symbols: Some(BTreeSet::from([symbol("a", 1).key(), symbol("b", 5).key()])),
    };
    let (ctx, _dir) = ctx(scope.clone(), Value::Object(Map::new()));
    assert_eq!(
        check.plan(&ctx, &scope).unwrap(),
        Plan {
            targets: 2,
            est_ms: 2
        }
    );
}

#[test]
fn scores_combine_by_maximum_across_languages() {
    let registry = registry();
    let check = registry.get(&CheckId::from(ID)).unwrap();
    let score = |value: f64, n: u64| Score {
        value,
        unit: "max".to_owned(),
        n,
    };
    assert_eq!(
        check.combine(&[score(42.0, 10), score(6.0, 1)]),
        Some(score(42.0, 11))
    );
    assert_eq!(check.combine(&[]), None);
}
