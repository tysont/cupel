// ABOUTME: static.crap over synthetic facts: CRAP from cyclomatic complexity and a hand-built CoverageMap, its
// ABOUTME: bands, no CRAP at all without coverage, test-file hints, settings, scope, and the score (ADR-067).
mod support;

use std::collections::BTreeSet;

use cupel_core::coverage::{CoverageMap, FileCoverage, LineHits};
use cupel_core::syntax::{
    Decision, DecisionKind, Span, SymbolDef, SymbolKind, TestConventions, TestPairing, Visibility,
};
use cupel_core::{
    ActionKind, Check, CheckId, CheckResult, CheckStatus, FactKind, Facts, Finding, FindingId,
    LangId, MetricPer, Plan, Scope, Score, Severity,
};
use serde_json::{Map, Value, json};
use support::{Recorded, combined, language};

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
            span: None,
            kind: DecisionKind::Branch,
            nesting: 0,
            cyclomatic: 1,
            cognitive: 1,
            table: false,
            continues: false,
            expression: false,
        })
        .collect()
}

/// Facts for `symbols`, each with its decisions, optionally the language's test conventions,
/// and optionally a coverage map.
struct World {
    symbols: Vec<(SymbolDef, Vec<Decision>)>,
    conventions: Option<TestConventions>,
    coverage: Option<CoverageMap>,
}

impl World {
    /// `symbols` with a coverage map in which every line of every symbol is instrumented and
    /// never run: coverage 0, so CRAP is `CC² + CC`.
    fn of(symbols: Vec<(SymbolDef, Vec<Decision>)>) -> Self {
        let coverage = uncovered(symbols.iter().map(|(symbol, _)| symbol));
        Self {
            symbols,
            conventions: None,
            coverage: Some(coverage),
        }
    }

    fn facts(&self, check: &dyn Check) -> Facts {
        let mut facts = support::decided_facts(check, &self.symbols);
        if let Some(conventions) = &self.conventions {
            facts.insert(conventions.clone()).unwrap();
        }
        if let Some(coverage) = &self.coverage {
            facts.insert(coverage.clone()).unwrap();
        }
        facts
    }

    fn scope(&self) -> Scope {
        support::decided_scope(&self.symbols)
    }
}

/// A coverage map with every line of `symbols` instrumented and run `hits` times.
fn covered<'a>(symbols: impl Iterator<Item = &'a SymbolDef>, hits: u64) -> CoverageMap {
    let mut files: Vec<FileCoverage> = Vec::new();
    for symbol in symbols {
        let lines = (symbol.span.line..=symbol.span.end_line).map(|line| LineHits { line, hits });
        match files.iter_mut().find(|file| file.file == symbol.file) {
            Some(file) => file.lines.extend(lines),
            None => files.push(FileCoverage {
                file: symbol.file.clone(),
                lines: lines.collect(),
                branches: None,
            }),
        }
    }
    CoverageMap { files }
}

fn uncovered<'a>(symbols: impl Iterator<Item = &'a SymbolDef>) -> CoverageMap {
    covered(symbols, 0)
}

/// Runs the check over `world` with `config`; its result and findings.
fn run(world: &World, config: Value) -> (CheckResult, Vec<Finding>) {
    run_in(world, world.scope(), config)
}

fn run_in(world: &World, scope: Scope, config: Value) -> (CheckResult, Vec<Finding>) {
    let (result, recorded) = recorded_in(world, scope, config);
    (result, recorded.findings)
}

/// Runs the check over `world`; its result and every finding and measurement it reported.
fn recorded_in(world: &World, scope: Scope, config: Value) -> (CheckResult, Recorded) {
    let check = support::check(ID);
    support::recorded(check, &world.facts(check), scope, config).unwrap()
}

#[test]
fn the_manifest_declares_its_facts_kinds_actions_and_metrics() {
    let manifest = support::check(ID).manifest();
    assert_eq!(
        support::Shape::of(manifest),
        support::Shape {
            needs: vec![FactKind::Symbols, FactKind::Decisions, FactKind::NormTree],
            wants: vec![FactKind::CoverageMap, FactKind::TestConventions],
            tier: 0,
            cost: cupel_core::CostClass::Quick,
            kinds: vec![
                ("crap.high", ActionKind::WriteTest),
                ("crap.watch", ActionKind::Review)
            ],
            metrics: vec![
                ("max", MetricPer::Scope),
                ("mean", MetricPer::Scope),
                ("crap", MetricPer::Symbol)
            ],
            evidence: vec!["object"; 2],
        }
    );
}

#[test]
fn without_coverage_crap_is_null_and_nothing_is_reported() {
    let mut world = World::of(vec![(symbol("risky", 1), branches(6))]);
    world.coverage = None;
    let (result, recorded) = recorded_in(&world, world.scope(), json!({}));
    assert!(recorded.findings.is_empty(), "{:?}", recorded.findings);
    assert!(recorded.measurements.is_empty());
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.skipped, None);
    assert_eq!(result.score, None);
    assert!(result.metrics.is_empty(), "{:?}", result.metrics);
    assert_eq!(result.findings, 0);
}

#[test]
fn coverage_lowers_crap_toward_cyclomatic_complexity() {
    // CRAP = CC² × (1 − coverage)³ + CC: CC 6 fully covered is 6; half covered 6 × 6 / 8 + 6.
    // Coverage is of the body: the declaration line (10) does not count (ADR-079).
    let full = symbol("full", 1);
    let half = symbol("half", 10);
    let mut coverage = covered([&full].into_iter(), 3);
    coverage.files[0].lines.extend([
        LineHits { line: 10, hits: 1 },
        LineHits { line: 11, hits: 1 },
        LineHits { line: 12, hits: 0 },
    ]);
    let mut world = World::of(vec![(full, branches(6)), (half, branches(6))]);
    world.coverage = Some(coverage);
    let (_, recorded) = recorded_in(&world, world.scope(), json!({}));
    assert_eq!(
        recorded.measured(),
        [("crap", "m::full", 6.0), ("crap", "m::half", 10.5)]
    );
    let evidence: Vec<(Value, Value)> = recorded
        .findings
        .iter()
        .map(|f| {
            (
                f.evidence.0["crap"].clone(),
                f.evidence.0["coverage"].clone(),
            )
        })
        .collect();
    assert_eq!(
        evidence,
        [(json!(6.0), json!(1.0)), (json!(10.5), json!(0.5))]
    );
}

#[test]
fn a_symbol_with_no_instrumented_line_has_no_crap() {
    let measured = symbol("measured", 1);
    let mut world = World::of(vec![
        (measured.clone(), branches(6)),
        (symbol("unmeasured", 10), branches(6)),
    ]);
    world.coverage = Some(uncovered([&measured].into_iter()));
    let (result, findings) = run(&world, json!({}));
    assert_eq!(
        support::kinds(&findings),
        [("m::measured".to_owned(), "crap.high".to_owned())]
    );
    assert_eq!(result.score.map(|score| score.n), Some(1));
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
    let (result, findings) = run(&world, json!({}));
    let expected: Vec<(String, String)> = [
        ("m::lowest_watch", "crap.watch"),
        ("m::highest_watch", "crap.watch"),
        ("m::lowest_high", "crap.high"),
        ("m::risky", "crap.high"),
    ]
    .iter()
    .map(|(s, k)| ((*s).to_owned(), (*k).to_owned()))
    .collect();
    assert_eq!(support::kinds(&findings), expected);
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
    let (_, findings) = run(&world, json!({}));
    let [finding] = findings.as_slice() else {
        panic!("one finding expected: {findings:?}")
    };
    let hash = support::hash_of_line(7);
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
        *json!({"crap": 42.0, "cyclomatic": 6, "coverage": 0.0, "watch": 5, "high": 30})
            .as_object()
            .unwrap()
    );
    assert!(finding.title.contains("CRAP 42"), "{}", finding.title);
    assert!(finding.action.hints.is_empty(), "no test conventions given");
}

#[test]
fn a_watch_finding_is_information_to_review_and_alone_leaves_the_check_passing() {
    let world = World::of(vec![(symbol("branchy", 1), branches(3))]);
    let (result, findings) = run(&world, json!({}));
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
    let (_, findings) = run(&world, json!({}));
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
    let (result, findings) = run(&world, json!({}));
    assert_eq!(
        support::kinds(&findings),
        [("m::risky".to_owned(), "crap.high".to_owned())]
    );
    assert_eq!(result.score.map(|score| score.n), Some(1));
}

#[test]
fn bands_come_from_the_configuration_the_context_carries() {
    let world = World::of(vec![(symbol("branchy", 1), branches(3))]);
    let kind = |config| {
        let (_, findings) = run(&world, config);
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
    let (result, findings) = run_in(&world, scope, json!({}));
    assert_eq!(
        support::kinds(&findings),
        [("m::inside".to_owned(), "crap.high".to_owned())]
    );
    let worst = result.score.unwrap();
    assert_eq!((worst.value, worst.n), (42.0, 1));
}

#[test]
fn nothing_in_scope_passes_with_no_symbols_scored() {
    let world = World::of(vec![]);
    let (result, findings) = run(&world, json!({}));
    assert!(findings.is_empty());
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.skipped, None);
    assert_eq!(result.score.map(|score| score.n), Some(0));
}

#[test]
fn unknown_settings_and_bad_values_are_configuration_errors() {
    let check = support::check(ID);
    for (config, key) in [
        (json!({"threshold": 3}), "threshold"),
        (json!({"no_coverage": true}), "no_coverage"),
        (json!({"high": -1}), "high"),
        (json!({"watch": 2.5}), "watch"),
    ] {
        let (named, valid) = support::setting_error(check, config);
        assert_eq!(named, key);
        assert_eq!(valid, ["high", "watch"]);
    }
}

#[test]
fn plans_count_the_symbols_in_scope_at_quick_cost() {
    let check = support::check(ID);
    let scope = Scope {
        files: vec![],
        symbols: Some(BTreeSet::from([symbol("a", 1).key(), symbol("b", 5).key()])),
    };
    let (ctx, _dir) = support::ctx(scope.clone(), Value::Object(Map::new()));
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
    let check = support::check(ID);
    let score = |value, n| support::score(value, "max", n);
    assert_eq!(
        check.combine(&[score(42.0, 10), score(6.0, 1)]),
        Some(score(42.0, 11))
    );
    assert_eq!(check.combine(&[]), None);
}

#[test]
fn the_result_reports_the_largest_and_mean_crap_of_the_scored_symbols() {
    let mut test = symbol("test_risky", 30);
    test.file = "f_test.x".to_owned();
    let mut world = World::of(vec![
        (symbol("risky", 1), branches(6)),
        (symbol("straight", 10), branches(1)),
        (test, branches(8)),
    ]);
    world.conventions = Some(TestConventions {
        test_globs: vec!["**/*_test.x".to_owned()],
        pairs: vec![],
    });
    let (result, _) = run(&world, json!({}));
    assert_eq!(
        result.metrics,
        [("max".to_owned(), 42.0), ("mean".to_owned(), 22.0)].into(),
        "CRAP 42 and 2; the test file is not scored"
    );
    let (empty, _) = run(&World::of(vec![]), json!({}));
    assert_eq!(
        empty.metrics,
        [("max".to_owned(), 0.0), ("mean".to_owned(), 0.0)].into()
    );
    world.coverage = None;
    let (unmeasured, _) = run(&world, json!({}));
    assert!(
        unmeasured.metrics.is_empty(),
        "without coverage there is no CRAP to report"
    );
}

#[test]
fn each_scored_symbol_is_measured_for_the_crap_ranking() {
    let world = World::of(vec![
        (symbol("risky", 1), branches(6)),
        (symbol("straight", 10), branches(1)),
    ]);
    let (_, recorded) = recorded_in(&world, world.scope(), json!({}));
    assert_eq!(
        recorded.measured(),
        [("crap", "m::risky", 42.0), ("crap", "m::straight", 2.0)]
    );
    let risky = &recorded.measurements[0].target;
    assert_eq!(risky.symbol, Some(support::hash_of_line(1)));
    let location = risky.location.as_ref().unwrap();
    assert_eq!((location.file.as_str(), location.line), ("f.x", 1));
}

#[test]
fn max_combines_by_the_largest_and_mean_by_the_mean_over_every_symbol() {
    let check = support::check(ID);
    let results = [
        language(check, &[("max", 42.0), ("mean", 10.0)], 1),
        language(check, &[("max", 30.0), ("mean", 40.0)], 3),
    ];
    assert_eq!(combined(check, "max", &results), Some(42.0));
    assert_eq!(
        combined(check, "mean", &results),
        Some(32.5),
        "(10 × 1 + 40 × 3) / 4"
    );
    let empty = [language(check, &[("max", 0.0), ("mean", 0.0)], 0)];
    assert_eq!(combined(check, "mean", &empty), Some(0.0));
}
