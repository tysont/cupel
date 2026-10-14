// ABOUTME: static.complexity over synthetic facts: metrics from Decisions and Symbols, thresholds and
// ABOUTME: their validation, finding identity and shape, scope filtering, and the score (ADR-042).
mod support;

use std::collections::{BTreeMap, BTreeSet};

use cupel_checks::register;
use cupel_core::config::ConfigError;
use cupel_core::syntax::{
    Decision, DecisionKind, Decisions, NormNode, NormTree, NormalizedSymbol, Span, SymbolDecisions,
    SymbolDef, SymbolKind, Symbols, Visibility,
};
use cupel_core::{
    ActionKind, Cancellation, Check, CheckCtx, CheckId, CheckRegistry, CheckResult, CheckStatus,
    Error, Event, FactKind, Facts, Finding, FindingId, LangId, MetricPer, Plan, Scope, Score,
    Severity, SymbolHash, Writer,
};
use serde_json::{Map, Value, json};
use support::Recorded;

/// A symbol in `f.x` starting at `line`, spanning `lines` lines, with `parameters`.
fn symbol(name: &str, line: u32, lines: u32, parameters: u32) -> SymbolDef {
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
            end_line: line + lines - 1,
            end_col: 2,
        },
        body: None,
        parameters,
    }
}

fn decision(kind: DecisionKind, nesting: u32, cyclomatic: u32, cognitive: u32) -> Decision {
    Decision {
        kind,
        nesting,
        cyclomatic,
        cognitive,
        table: false,
    }
}

fn hash_of(line: u32) -> SymbolHash {
    format!("sh_{line:016x}").parse().unwrap()
}

/// Facts for `symbols`, each with its decisions.
struct World {
    symbols: Vec<(SymbolDef, Vec<Decision>)>,
}

impl World {
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
        facts
    }

    fn scope(&self) -> Scope {
        Scope {
            files: vec!["f.x".to_owned()],
            symbols: Some(self.symbols.iter().map(|(s, _)| s.key()).collect()),
        }
    }
}

fn complexity() -> CheckRegistry {
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

/// Runs the check over `world` with `config`; its result and findings.
fn run(world: &World, config: Value) -> (CheckResult, Vec<Finding>) {
    run_in(world, world.scope(), config)
}

fn run_in(world: &World, scope: Scope, config: Value) -> (CheckResult, Vec<Finding>) {
    let registry = complexity();
    let check = registry.get(&CheckId::from("static.complexity")).unwrap();
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
    let registry = complexity();
    let manifest = registry
        .get(&CheckId::from("static.complexity"))
        .unwrap()
        .manifest();
    assert_eq!(
        manifest.needs,
        [FactKind::Symbols, FactKind::Decisions, FactKind::NormTree]
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
            ("complexity.high", ActionKind::ReduceComplexity),
            ("nesting.deep", ActionKind::ReduceComplexity),
            ("params.many", ActionKind::RefactorExtract),
            ("function.long", ActionKind::RefactorExtract),
        ]
    );
    let metrics: Vec<(&str, MetricPer)> = manifest
        .metrics
        .iter()
        .map(|m| (m.name.as_str(), m.per))
        .collect();
    assert_eq!(
        metrics,
        [
            ("cyclomatic_max", MetricPer::Scope),
            ("cognitive_max", MetricPer::Scope),
            ("nesting_max", MetricPer::Scope),
            ("params_max", MetricPer::Scope),
            ("length_max", MetricPer::Scope),
            ("cyclomatic", MetricPer::Symbol),
            ("cognitive", MetricPer::Symbol),
        ]
    );
    for spec in &manifest.findings {
        assert_eq!(spec.evidence["type"], json!("object"), "{}", spec.kind);
    }
}

#[test]
fn metrics_follow_the_weights_and_nesting_the_producer_attached() {
    use DecisionKind::{BooleanOp, Branch, Case, EarlyReturn, Loop};
    let world = World {
        symbols: vec![(
            symbol("busy", 1, 10, 1),
            vec![
                decision(Branch, 0, 1, 1),
                decision(Loop, 1, 1, 1),
                decision(BooleanOp, 2, 1, 1),
                decision(Case, 2, 1, 1),
                decision(Case, 2, 1, 0),
                decision(EarlyReturn, 3, 0, 1),
            ],
        )],
    };
    let (_, findings) = run(
        &world,
        json!({"cyclomatic_max": 0, "cognitive_max": 0, "nesting_max": 0}),
    );
    let high = findings
        .iter()
        .find(|f| f.kind == "complexity.high")
        .unwrap();
    // 1 + 1 + 1 + 1 + 1 + 1 + 0; (1 + 0) + (1 + 1) + 1 + (1 + 2) + 0 + 1.
    assert_eq!(high.evidence.0["cyclomatic"], json!(6));
    assert_eq!(high.evidence.0["cognitive"], json!(8));
    let deep = findings.iter().find(|f| f.kind == "nesting.deep").unwrap();
    assert_eq!(deep.evidence.0["depth"], json!(3), "the case at nesting 2");
}

#[test]
fn each_kind_fires_only_above_its_default_threshold() {
    use DecisionKind::{Branch, Loop};
    let branches = |n: u32| vec![decision(Branch, 0, 1, 1); n as usize];
    let nested = |depth: u32| {
        (0..depth)
            .map(|level| decision(Loop, level, 1, 1))
            .collect::<Vec<_>>()
    };
    let world = World {
        symbols: vec![
            (symbol("at_cyclomatic", 1, 5, 0), branches(9)),
            (symbol("over_cyclomatic", 10, 5, 0), branches(10)),
            (symbol("at_nesting", 20, 5, 0), nested(4)),
            (symbol("over_nesting", 30, 5, 0), nested(5)),
            (symbol("at_params", 40, 5, 5), vec![]),
            (symbol("over_params", 50, 5, 6), vec![]),
            (symbol("at_length", 60, 60, 0), vec![]),
            (symbol("over_length", 130, 61, 0), vec![]),
        ],
    };
    let (result, findings) = run(&world, json!({}));
    let expected: Vec<(String, String)> = [
        ("m::over_cyclomatic", "complexity.high"),
        ("m::over_nesting", "nesting.deep"),
        ("m::over_params", "params.many"),
        ("m::over_length", "function.long"),
    ]
    .iter()
    .map(|(s, k)| ((*s).to_owned(), (*k).to_owned()))
    .collect();
    assert_eq!(kinds(&findings), expected);
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.findings, 4);
    assert_eq!(
        result.score,
        Some(Score {
            value: 11.0,
            unit: "cyclomatic_max".to_owned(),
            n: 8
        })
    );
}

#[test]
fn a_finding_is_identified_located_and_reproducible_by_its_symbol_hash() {
    let world = World {
        symbols: vec![(symbol("wide", 7, 3, 9), vec![])],
    };
    let (_, findings) = run(&world, json!({}));
    let [finding] = findings.as_slice() else {
        panic!("one finding expected: {findings:?}")
    };
    let hash = hash_of(7);
    let check = CheckId::from("static.complexity");
    assert_eq!(
        finding.id,
        FindingId::compute(&check, "params.many", &hash, None)
    );
    assert_eq!(finding.check, check);
    assert_eq!(finding.severity, Severity::Warn);
    assert_eq!(finding.tier.value(), 0);
    assert_eq!(finding.symbol.as_ref().unwrap().hash, hash);
    assert_eq!(finding.symbol.as_ref().unwrap().lang, LangId::from("x"));
    let location = finding.location.as_ref().unwrap();
    assert_eq!(
        (location.file.as_str(), location.line, location.end_line),
        ("f.x", 7, 9)
    );
    assert_eq!(finding.action.kind, ActionKind::RefactorExtract);
    assert_eq!(
        finding.repro,
        format!("cupel check --only static.complexity --symbols {hash}")
    );
    assert_eq!(
        finding.suppress,
        format!("cupel suppress {} --reason '…'", finding.id)
    );
    assert_eq!(
        finding.evidence.0,
        *json!({"parameters": 9, "params_max": 5})
            .as_object()
            .unwrap()
    );
}

#[test]
fn only_symbols_in_scope_are_measured() {
    let world = World {
        symbols: vec![
            (symbol("inside", 1, 3, 9), vec![]),
            (symbol("outside", 10, 3, 9), vec![]),
        ],
    };
    let scope = Scope {
        files: vec!["f.x".to_owned()],
        symbols: Some(BTreeSet::from([world.symbols[0].0.key()])),
    };
    let (result, findings) = run_in(&world, scope, json!({}));
    assert_eq!(
        kinds(&findings),
        [("m::inside".to_owned(), "params.many".to_owned())]
    );
    assert_eq!(result.score.unwrap().n, 1);
}

#[test]
fn thresholds_come_from_the_configuration_the_context_carries() {
    let world = World {
        symbols: vec![(symbol("wide", 1, 3, 6), vec![])],
    };
    assert_eq!(run(&world, json!({"params_max": 6})).1.len(), 0);
    assert_eq!(run(&world, json!({"params_max": 2})).1.len(), 1);
}

#[test]
fn unknown_settings_and_bad_values_are_configuration_errors() {
    let registry = complexity();
    let check = registry.get(&CheckId::from("static.complexity")).unwrap();
    for (config, key) in [
        (json!({"cyclomatic": 3}), "cyclomatic"),
        (json!({"params_max": -1}), "params_max"),
        (json!({"length_max": "long"}), "length_max"),
        (json!({"nesting_max": 2.5}), "nesting_max"),
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
        assert!(valid.contains(&"cyclomatic_max".to_owned()));
    }
}

#[test]
fn plans_count_the_symbols_in_scope_at_quick_cost() {
    let registry = complexity();
    let check = registry.get(&CheckId::from("static.complexity")).unwrap();
    let scope = Scope {
        files: vec![],
        symbols: Some(BTreeSet::from([
            symbol("a", 1, 1, 0).key(),
            symbol("b", 5, 1, 0).key(),
        ])),
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
    let registry = complexity();
    let check = registry.get(&CheckId::from("static.complexity")).unwrap();
    let score = |value: f64, n: u64| Score {
        value,
        unit: "cyclomatic_max".to_owned(),
        n,
    };
    assert_eq!(
        check.combine(&[score(4.0, 10), score(12.0, 1)]),
        Some(score(12.0, 11))
    );
    assert_eq!(check.combine(&[]), None);
}

/// Two symbols: `wide` (cyclomatic 3, cognitive 3, nesting 2, 7 parameters, 12 lines) and
/// `small` (straight-line, 1 parameter, 3 lines).
fn two_symbols() -> World {
    use DecisionKind::{Branch, Loop};
    World {
        symbols: vec![
            (
                symbol("wide", 1, 12, 7),
                vec![decision(Branch, 0, 1, 1), decision(Loop, 1, 1, 1)],
            ),
            (symbol("small", 20, 3, 1), vec![]),
        ],
    }
}

#[test]
fn the_result_reports_each_scope_metric_as_the_largest_value_in_scope() {
    let (result, _) = run(&two_symbols(), json!({}));
    let expected: BTreeMap<String, f64> = [
        ("cyclomatic_max", 3.0),
        ("cognitive_max", 3.0),
        ("nesting_max", 2.0),
        ("params_max", 7.0),
        ("length_max", 12.0),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value))
    .collect();
    assert_eq!(result.metrics, expected);
    let (empty, _) = run(&World { symbols: vec![] }, json!({}));
    assert!(
        empty.metrics.values().all(|value| *value == 0.0),
        "{:?}",
        empty.metrics
    );
}

#[test]
fn each_symbol_in_scope_is_measured_for_cyclomatic_and_cognitive_rankings() {
    let world = two_symbols();
    let registry = complexity();
    let check = registry.get(&CheckId::from("static.complexity")).unwrap();
    let (ctx, _dir) = ctx(world.scope(), json!({}));
    let plan = check.plan(&ctx, &ctx.scope).unwrap();
    let mut sink = Recorded::default();
    check
        .run(&ctx, &world.facts(check), &plan, &mut sink)
        .unwrap();
    assert_eq!(
        sink.measured(),
        [
            ("cyclomatic", "m::wide", 3.0),
            ("cognitive", "m::wide", 3.0),
            ("cyclomatic", "m::small", 1.0),
            ("cognitive", "m::small", 0.0),
        ]
    );
    let wide = &sink.measurements[0].target;
    assert_eq!(wide.symbol, Some(hash_of(1)));
    let location = wide.location.as_ref().unwrap();
    assert_eq!(
        (location.file.as_str(), location.line, location.end_line),
        ("f.x", 1, 12)
    );
}
