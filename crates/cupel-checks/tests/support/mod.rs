// ABOUTME: Helpers shared by the cupel-checks integration tests: registered checks, running one over facts,
// ABOUTME: the facts and values tests build, results to combine, and a manifest's shape. Included with `mod support;`.
#![allow(
    dead_code,
    reason = "each test crate uses a different subset of the helpers"
)]

use std::collections::BTreeMap;
use std::sync::OnceLock;

use cupel_core::config::ConfigError;
use cupel_core::syntax::{
    Decision, Decisions, ImportTarget, NormNode, NormTree, NormalizedSymbol, Span, SymbolDecisions,
    SymbolDef, Symbols,
};
use cupel_core::{
    ActionKind, Cancellation, Check, CheckCtx, CheckId, CheckRegistry, CheckResult, CheckStatus,
    CostClass, Error, Event, EventSink, Fact, FactKind, Facts, Finding, Manifest, Measurement,
    MetricPer, RunId, Scope, Score, SymbolHash, Tier, Writer,
};
use serde_json::Value;

/// Keeps every finding and measurement a check reports.
#[derive(Default)]
pub struct Recorded {
    pub findings: Vec<Finding>,
    pub measurements: Vec<Measurement>,
}

impl EventSink for Recorded {
    fn emit(&mut self, event: Event) {
        if let Event::Finding(finding) = event {
            self.findings.push(*finding);
        }
    }

    fn measure(&mut self, measurement: Measurement) {
        self.measurements.push(measurement);
    }
}

impl Recorded {
    /// Each measurement as (metric, target display, value), in the order reported.
    pub fn measured(&self) -> Vec<(&str, &str, f64)> {
        self.measurements
            .iter()
            .map(|m| (m.metric.as_str(), m.target.display.as_str(), m.value))
            .collect()
    }
}

/// One language's result of `check`: `metrics`, over a score covering `n` targets.
pub fn language(check: &dyn Check, metrics: &[(&str, f64)], n: u64) -> CheckResult {
    CheckResult {
        id: check.manifest().id.clone(),
        status: CheckStatus::Pass,
        tier: Tier::try_from(0).unwrap(),
        cost: CostClass::Quick,
        score: Some(Score {
            value: 0.0,
            unit: String::new(),
            n,
        }),
        metrics: metrics
            .iter()
            .map(|(name, value)| ((*name).to_owned(), *value))
            .collect::<BTreeMap<_, _>>(),
        delta: None,
        findings: 0,
        duration_ms: 0,
        skipped: None,
        oracle: None,
        notes: Vec::new(),
    }
}

/// `name` combined across `results` the way the scheduler does: over the results reporting it.
pub fn combined(check: &dyn Check, name: &str, results: &[CheckResult]) -> Option<f64> {
    let reported: Vec<&CheckResult> = results
        .iter()
        .filter(|result| result.metrics.contains_key(name))
        .collect();
    check.combine_metric(name, &reported)
}

/// What a manifest declares: the facts it needs and wants, its tier and cost, each finding kind
/// with its action, each metric with what it is per, and the JSON type of each kind's evidence.
#[derive(Debug, PartialEq)]
pub struct Shape<'a> {
    pub needs: Vec<FactKind>,
    pub wants: Vec<FactKind>,
    pub tier: u8,
    pub cost: CostClass,
    pub kinds: Vec<(&'a str, ActionKind)>,
    pub metrics: Vec<(&'a str, MetricPer)>,
    pub evidence: Vec<&'a str>,
}

impl<'a> Shape<'a> {
    pub fn of(manifest: &'a Manifest) -> Self {
        Self {
            needs: manifest.needs.clone(),
            wants: manifest.wants.clone(),
            tier: manifest.tier.value(),
            cost: manifest.cost,
            kinds: manifest
                .findings
                .iter()
                .map(|spec| (spec.kind.as_str(), spec.action))
                .collect(),
            metrics: manifest
                .metrics
                .iter()
                .map(|metric| (metric.name.as_str(), metric.per))
                .collect(),
            evidence: manifest
                .findings
                .iter()
                .map(|spec| spec.evidence["type"].as_str().unwrap_or("?"))
                .collect(),
        }
    }
}

/// Every native check, registered.
pub fn registry() -> CheckRegistry {
    let mut registry = CheckRegistry::default();
    cupel_checks::register(&mut registry);
    registry
}

/// The registered check `id`.
pub fn check(id: &str) -> &'static dyn Check {
    static REGISTRY: OnceLock<CheckRegistry> = OnceLock::new();
    REGISTRY
        .get_or_init(registry)
        .get(&CheckId::from(id))
        .unwrap_or_else(|| panic!("{id} is registered"))
}

/// Plans and runs `check` over `facts` in `scope` with `config` (a table); its result and every
/// finding and measurement it reported.
pub fn recorded(
    check: &dyn Check,
    facts: &Facts,
    scope: Scope,
    config: Value,
) -> cupel_core::Result<(CheckResult, Recorded)> {
    let (ctx, _dir) = ctx(scope, config);
    let plan = check.plan(&ctx, &ctx.scope)?;
    let mut recorded = Recorded::default();
    let result = check.run(&ctx, facts, &plan, &mut recorded)?;
    Ok((result, recorded))
}

/// The result and findings of the check `id` run over `fact` alone, in a scope of `src/lib.rs`.
pub fn run_over<F: Fact>(id: &str, fact: F) -> (CheckResult, Vec<Finding>) {
    let check = check(id);
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(fact).unwrap();
    let scope = Scope::of_files(vec!["src/lib.rs".to_owned()]);
    let (result, recorded) = recorded(check, &facts, scope, serde_json::json!({})).unwrap();
    (result, recorded.findings)
}

/// The setting `check` names, and the settings it lists as valid, when planning with `config`
/// fails as a configuration error.
pub fn setting_error(check: &dyn Check, config: Value) -> (String, Vec<String>) {
    let (ctx, _dir) = ctx(Scope::default(), config);
    let error = check.plan(&ctx, &Scope::default()).unwrap_err();
    let Error::Config(ConfigError::CheckSetting { key, valid, .. }) = error else {
        panic!("expected a check-setting error, got {error:?}")
    };
    (key, valid)
}

/// Asserts the check `id` has no settings: any key is a configuration error that lists none.
pub fn assert_refuses_settings(id: &str) {
    let (key, valid) = setting_error(check(id), serde_json::json!({"anything": 1}));
    assert_eq!((key.as_str(), valid), ("anything", Vec::<String>::new()));
}

/// Asserts that each of `metrics` of the check `id` adds up across two languages, and so do
/// its scores in `unit`.
pub fn assert_counts_add_up(id: &str, metrics: &[&str], unit: &str) {
    let check = check(id);
    let values =
        |base: f64| -> Vec<(&str, f64)> { metrics.iter().map(|metric| (*metric, base)).collect() };
    let results = [
        language(check, &values(2.0), 4),
        language(check, &values(1.0), 3),
    ];
    let summed: Vec<Option<f64>> = metrics
        .iter()
        .map(|metric| combined(check, metric, &results))
        .collect();
    assert_eq!(summed, vec![Some(3.0); metrics.len()]);
    assert_eq!(
        check.combine(&[score(2.0, unit, 4), score(1.0, unit, 3)]),
        Some(score(3.0, unit, 7))
    );
}

/// A score of `value` in `unit` over `n` targets.
pub fn score(value: f64, unit: &str, n: u64) -> Score {
    Score {
        value,
        unit: unit.to_owned(),
        n,
    }
}

/// A span of `lines` lines from `line`, from column 1 to column 2.
pub fn span(line: u32, lines: u32) -> Span {
    Span {
        line,
        col: 1,
        end_line: line + lines - 1,
        end_col: 2,
    }
}

/// A hash unique to a symbol's first line.
pub fn hash_of_line(line: u32) -> SymbolHash {
    format!("sh_{line:016x}").parse().unwrap()
}

/// `Symbols`, `Decisions`, and a one-node `NormTree` (hashed by line) for `symbols`, each with
/// its decisions, declared as `check` declares them.
pub fn decided_facts(check: &dyn Check, symbols: &[(SymbolDef, Vec<Decision>)]) -> Facts {
    let mut facts = Facts::for_manifest(check.manifest());
    facts
        .insert(Symbols {
            symbols: symbols.iter().map(|(s, _)| s.clone()).collect(),
        })
        .unwrap();
    facts
        .insert(Decisions {
            symbols: symbols
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
            symbols: symbols
                .iter()
                .map(|(s, _)| NormalizedSymbol {
                    symbol: s.key(),
                    hash: hash_of_line(s.span.line),
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

/// The scope of `symbols`, all in one file.
pub fn decided_scope(symbols: &[(SymbolDef, Vec<Decision>)]) -> Scope {
    Scope {
        files: vec!["f.x".to_owned()],
        symbols: Some(symbols.iter().map(|(s, _)| s.key()).collect()),
    }
}

/// An import of the file at `path`.
pub fn imported_file(path: &str) -> ImportTarget {
    ImportTarget::File {
        path: path.to_owned(),
    }
}

/// An import of the external `package`.
pub fn imported_package(package: &str) -> ImportTarget {
    ImportTarget::External {
        package: package.to_owned(),
    }
}

/// A check context over `scope` with `config` (a table), writing under a temporary directory
/// that lives as long as the returned guard.
pub fn ctx(scope: Scope, config: Value) -> (CheckCtx, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let writer = Writer::new(dir.path(), false).unwrap();
    let Value::Object(config) = config else {
        panic!("config is a table")
    };
    let ctx = CheckCtx {
        scope,
        config,
        writer: writer.scratch(&RunId::generate()),
        cancel: Cancellation::new(),
    };
    (ctx, dir)
}

/// Each finding as (symbol display, kind).
pub fn kinds(findings: &[Finding]) -> Vec<(String, String)> {
    findings
        .iter()
        .map(|f| (f.symbol.as_ref().unwrap().display.clone(), f.kind.clone()))
        .collect()
}

/// `expected` as owned (display, kind) pairs, to compare with `kinds`.
pub fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(display, kind)| ((*display).to_owned(), (*kind).to_owned()))
        .collect()
}
