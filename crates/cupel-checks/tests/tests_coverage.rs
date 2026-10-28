// ABOUTME: tests.coverage over synthetic facts: uncovered symbols, uncovered branches only where the map measured
// ABOUTME: branches (else the `branches: unavailable` note), test code and unmeasured symbols, metrics (ADR-079).
mod support;

use cupel_core::coverage::{CoverageMap, FileCoverage, Listing};
use cupel_core::syntax::{
    NormNode, NormTree, NormalizedSymbol, Span, SymbolDef, SymbolKind, Symbols, TestConventions,
    TestPairing, Visibility,
};
use cupel_core::{
    ActionKind, Check, CheckId, CheckResult, CheckStatus, CostClass, Error, FactKind, Facts,
    Finding, FindingId, LangId, MetricPer, ProtectionMetric, Scope, Severity,
};
use serde_json::{Value, json};
use support::{Recorded, combined, language};

const ID: &str = "tests.coverage";

/// A function `name` in `file` on lines `line` to `line + 4`.
fn symbol(name: &str, file: &str, line: u32) -> SymbolDef {
    SymbolDef {
        name: name.to_owned(),
        namespace: "m".to_owned(),
        display: format!("m::{name}"),
        kind: SymbolKind::Function,
        visibility: Visibility::Public,
        lang: LangId::from("x"),
        file: file.to_owned(),
        span: Span {
            line,
            col: 1,
            end_line: line + 4,
            end_col: 2,
        },
        body: None,
        parameters: 0,
    }
}

/// The symbols, coverage, and optionally test conventions a run sees.
struct World {
    symbols: Vec<SymbolDef>,
    coverage: Option<CoverageMap>,
    conventions: Option<TestConventions>,
}

impl World {
    fn new(symbols: Vec<SymbolDef>, coverage: CoverageMap) -> Self {
        Self {
            symbols,
            coverage: Some(coverage),
            conventions: None,
        }
    }

    fn facts(&self, check: &dyn Check) -> Facts {
        let mut facts = Facts::for_manifest(check.manifest());
        facts
            .insert(Symbols {
                symbols: self.symbols.clone(),
            })
            .unwrap();
        facts
            .insert(NormTree {
                symbols: self
                    .symbols
                    .iter()
                    .map(|symbol| NormalizedSymbol {
                        symbol: symbol.key(),
                        hash: support::hash_of_line(symbol.span.line),
                        root: NormNode {
                            span: None,
                            text: None,
                            label: "fn".to_owned(),
                            children: Vec::new(),
                        },
                    })
                    .collect(),
            })
            .unwrap();
        if let Some(coverage) = &self.coverage {
            facts.insert(coverage.clone()).unwrap();
        }
        if let Some(conventions) = &self.conventions {
            facts.insert(conventions.clone()).unwrap();
        }
        facts
    }

    fn scope(&self) -> Scope {
        let mut files: Vec<String> = self.symbols.iter().map(|s| s.file.clone()).collect();
        files.dedup();
        Scope {
            files,
            symbols: Some(self.symbols.iter().map(SymbolDef::key).collect()),
        }
    }
}

fn run_in(world: &World, scope: Scope) -> cupel_core::Result<(CheckResult, Recorded)> {
    let check = support::check(ID);
    support::recorded(check, &world.facts(check), scope, json!({}))
}

fn run(world: &World) -> (CheckResult, Vec<Finding>) {
    let (result, recorded) = run_in(world, world.scope()).unwrap();
    (result, recorded.findings)
}

/// `a` (lines 1–5) never ran; `b` (lines 10–14) ran but one of its two branches never did;
/// `c` (lines 20–24) ran fully. Each has its declaration line and two body lines instrumented.
/// Branches are measured when `measured`.
fn three_symbols(measured: bool) -> World {
    let branch_hits: &[(u32, u32, u64)] = &[(11, 0, 3), (11, 1, 0), (21, 0, 1), (21, 1, 1)];
    World::new(
        vec![
            symbol("a", "f.x", 1),
            symbol("b", "f.x", 10),
            symbol("c", "f.x", 20),
        ],
        CoverageMap::new(vec![FileCoverage::from_hits(
            "f.x",
            &[
                (1, 0),
                (2, 0),
                (3, 0),
                (10, 3),
                (11, 3),
                (12, 0),
                (20, 1),
                (21, 1),
                (22, 1),
            ],
            measured.then_some(branch_hits),
        )]),
    )
}

#[test]
fn the_manifest_declares_its_facts_kinds_actions_and_metrics() {
    let manifest = support::check(ID).manifest();
    assert_eq!(
        support::Shape::of(manifest),
        support::Shape {
            needs: vec![FactKind::Symbols, FactKind::CoverageMap],
            wants: vec![FactKind::NormTree, FactKind::TestConventions],
            tier: 2,
            cost: CostClass::Standard,
            kinds: vec![
                ("coverage.uncovered-symbol", ActionKind::WriteTest),
                ("coverage.uncovered-branch", ActionKind::WriteTest),
                ("coverage.unloaded-file", ActionKind::WriteTest),
            ],
            metrics: vec![
                ("line", MetricPer::Scope),
                ("branch", MetricPer::Scope),
                ("lines", MetricPer::Scope),
                ("branches", MetricPer::Scope),
                ("uncovered", MetricPer::Scope),
                ("uncovered_lines", MetricPer::Symbol),
                ("line_coverage", MetricPer::Symbol),
            ],
            evidence: vec!["object"; 3],
        }
    );
    let protection: Vec<(&str, Option<ProtectionMetric>)> = manifest
        .metrics
        .iter()
        .filter(|metric| metric.protection.is_some())
        .map(|metric| (metric.name.as_str(), metric.protection))
        .collect();
    assert_eq!(
        protection,
        [("line_coverage", Some(ProtectionMetric::Coverage))]
    );
}

#[test]
fn a_symbol_none_of_whose_lines_ran_is_an_uncovered_symbol_warning() {
    let (result, findings) = run(&three_symbols(true));
    let uncovered: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.kind == "coverage.uncovered-symbol")
        .collect();
    let [finding] = uncovered.as_slice() else {
        panic!("one uncovered symbol: {findings:?}");
    };
    assert_eq!(finding.symbol.as_ref().unwrap().display, "m::a");
    assert_eq!(finding.severity, Severity::Warn);
    assert_eq!(finding.action.kind, ActionKind::WriteTest);
    assert_eq!(
        finding.id,
        FindingId::compute(
            &CheckId::from(ID),
            "coverage.uncovered-symbol",
            &support::hash_of_line(1),
            None
        )
    );
    assert_eq!(
        Value::Object(finding.evidence.0.clone()),
        json!({"lines": 2, "covered": 0})
    );
    assert_eq!(
        result.status,
        CheckStatus::Fail,
        "an uncovered symbol fails"
    );
}

#[test]
fn a_symbol_that_ran_with_a_branch_that_never_did_is_an_uncovered_branch_note() {
    let (_, findings) = run(&three_symbols(true));
    let branches: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.kind == "coverage.uncovered-branch")
        .collect();
    let [finding] = branches.as_slice() else {
        panic!("one uncovered branch: {findings:?}");
    };
    assert_eq!(finding.symbol.as_ref().unwrap().display, "m::b");
    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(
        Value::Object(finding.evidence.0.clone()),
        json!({"branches": 2, "uncovered": 1, "lines": [11]})
    );
}

#[test]
fn without_measured_branches_there_are_no_branch_findings_and_the_result_says_so() {
    let (result, findings) = run(&three_symbols(false));
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("m::a", "coverage.uncovered-symbol")])
    );
    assert_eq!(result.notes, ["branches: unavailable"]);
    assert!(
        !result.metrics.contains_key("branch"),
        "{:?}",
        result.metrics
    );
    assert!(!result.metrics.contains_key("branches"));
    let (measured, _) = run(&three_symbols(true));
    assert!(measured.notes.is_empty(), "{:?}", measured.notes);
}

#[test]
fn a_declaration_line_that_ran_without_its_body_is_not_coverage() {
    // A Python `def` line runs when the module is imported; the function ran only if its body did.
    let world = World::new(
        vec![symbol("imported", "f.x", 1), symbol("one_line", "f.x", 10)],
        CoverageMap::new(vec![FileCoverage::from_hits(
            "f.x",
            &[(1, 1), (2, 0), (3, 0), (10, 1)],
            None,
        )]),
    );
    let (result, findings) = run(&world);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("m::imported", "coverage.uncovered-symbol")])
    );
    assert_eq!(
        Value::Object(findings[0].evidence.0.clone()),
        json!({"lines": 2, "covered": 0})
    );
    assert_eq!(
        result.score.unwrap().n,
        3,
        "a one-line function is measured by its one line"
    );
}

#[test]
fn branch_findings_need_branches_measured_in_the_symbols_own_file() {
    let mut world = World::new(
        vec![symbol("b", "f.x", 10), symbol("g", "g.x", 1)],
        CoverageMap::new(vec![
            FileCoverage::from_hits("f.x", &[(10, 1), (11, 1)], None),
            FileCoverage::from_hits("g.x", &[(1, 1), (2, 1)], Some(&[(2, 0, 1), (2, 1, 0)])),
        ]),
    );
    world.symbols[0].span.end_line = 12;
    let (result, findings) = run(&world);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("m::g", "coverage.uncovered-branch")])
    );
    assert!(result.notes.is_empty());
}

#[test]
fn the_result_measures_line_and_branch_coverage_of_the_scored_symbols() {
    let (result, recorded) = run_in(&three_symbols(true), three_symbols(true).scope()).unwrap();
    // Body lines: a 0/2, b 1/2, c 2/2 → 3/6. Branches: b 1/2, c 2/2 → 3/4.
    let score = result.score.as_ref().unwrap();
    assert_eq!(
        (score.value, score.unit.as_str(), score.n),
        (0.5, "line_ratio", 6)
    );
    assert_eq!(
        result.metrics,
        [
            ("branch", 0.75),
            ("branches", 4.0),
            ("line", 0.5),
            ("lines", 6.0),
            ("uncovered", 1.0)
        ]
        .map(|(name, value)| (name.to_owned(), value))
        .into()
    );
    assert_eq!(result.findings, 2);
    assert_eq!(
        recorded.measured(),
        [
            ("uncovered_lines", "m::a", 2.0),
            ("line_coverage", "m::a", 0.0),
            ("uncovered_lines", "m::b", 1.0),
            ("line_coverage", "m::b", 0.5),
            ("uncovered_lines", "m::c", 0.0),
            ("line_coverage", "m::c", 1.0),
        ]
    );
}

#[test]
fn symbols_without_an_instrumented_line_or_in_test_files_are_not_scored() {
    let mut world = World::new(
        vec![
            symbol("unmeasured", "f.x", 40),
            symbol("a", "f.x", 1),
            symbol("a_test", "f_test.x", 1),
        ],
        CoverageMap::new(vec![
            FileCoverage::from_hits("f.x", &[(1, 0)], None),
            FileCoverage::from_hits("f_test.x", &[(1, 0)], None),
        ]),
    );
    world.conventions = Some(TestConventions {
        test_globs: vec!["**/*_test.x".to_owned(), "*_test.x".to_owned()],
        pairs: vec![TestPairing {
            source: "f.x".to_owned(),
            tests: vec!["f_test.x".to_owned()],
        }],
    });
    let (result, findings) = run(&world);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("m::a", "coverage.uncovered-symbol")])
    );
    assert_eq!(
        findings[0].action.hints["nearest_test_file"],
        json!("f_test.x")
    );
    assert_eq!(result.score.unwrap().n, 1, "one instrumented line scored");
}

/// A map that lists only the files the suite loaded (ADR-152): a symbol in a file it leaves out
/// is an unloaded file, its span's lines all uncovered; the same map listing every file leaves
/// the symbol unmeasured, as does a loaded-only map that lists nothing at all.
#[test]
fn a_symbol_in_a_file_a_loaded_only_map_leaves_out_is_an_unloaded_file() {
    let symbols = vec![symbol("a", "f.x", 1), symbol("never", "g.x", 1)];
    let map = CoverageMap::new(vec![FileCoverage::from_hits("f.x", &[(2, 1)], None)]);
    let (result, findings) = run(&World::new(
        symbols.clone(),
        map.clone().listed(Listing::Loaded),
    ));
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("m::never", "coverage.unloaded-file")])
    );
    assert_eq!(findings[0].severity, Severity::Warn);
    assert_eq!(findings[0].action.kind, ActionKind::WriteTest);
    assert_eq!(
        Value::Object(findings[0].evidence.0.clone()),
        json!({"lines": 4, "covered": 0}),
        "the body's four lines stand in for the instrumented ones"
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(
        result.metrics.get("lines"),
        Some(&5.0),
        "a's one line and never's four"
    );
    assert_eq!(result.metrics.get("line"), Some(&0.2));
    assert_eq!(result.metrics.get("uncovered"), Some(&1.0));
    let (result, findings) = run(&World::new(symbols.clone(), map));
    assert!(
        findings.is_empty(),
        "listing every file, g.x has nothing to measure"
    );
    assert_eq!(result.score.unwrap().n, 1);
    let (_, findings) = run(&World::new(
        symbols,
        CoverageMap::default().listed(Listing::Loaded),
    ));
    assert!(
        findings.is_empty(),
        "a map that lists nothing says nothing about g.x"
    );
}

#[test]
fn only_symbols_in_scope_are_scored() {
    let world = three_symbols(true);
    let mut scope = world.scope();
    scope.symbols = Some(world.symbols[2..].iter().map(SymbolDef::key).collect());
    let (result, recorded) = run_in(&world, scope).unwrap();
    assert!(recorded.findings.is_empty());
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.metrics.get("line"), Some(&1.0));
}

#[test]
fn nothing_scored_passes_with_a_score_over_no_lines() {
    let world = World::new(vec![symbol("a", "f.x", 1)], CoverageMap::default());
    let (result, findings) = run(&world);
    assert!(findings.is_empty());
    assert_eq!(result.status, CheckStatus::Pass);
    let score = result.score.unwrap();
    assert_eq!((score.value, score.n), (0.0, 0));
}

#[test]
fn without_a_coverage_map_the_check_cannot_run() {
    let mut world = three_symbols(true);
    world.coverage = None;
    let error = run_in(&world, world.scope()).err();
    assert_eq!(
        error,
        Some(Error::AbsentFact {
            kind: FactKind::CoverageMap
        })
    );
}

#[test]
fn any_setting_is_a_configuration_error() {
    let (key, valid) = support::setting_error(support::check(ID), json!({"threshold": 80}));
    assert_eq!(key, "threshold");
    assert!(valid.is_empty());
}

#[test]
fn ratios_combine_weighted_by_what_they_are_ratios_of_and_counts_add_up() {
    let check = support::check(ID);
    let results = [
        language(
            check,
            &[
                ("line", 1.0),
                ("lines", 30.0),
                ("uncovered", 0.0),
                ("branch", 0.5),
                ("branches", 4.0),
            ],
            30,
        ),
        language(
            check,
            &[("line", 0.5), ("lines", 10.0), ("uncovered", 2.0)],
            10,
        ),
    ];
    assert_eq!(combined(check, "line", &results), Some(0.875));
    assert_eq!(combined(check, "branch", &results), Some(0.5));
    assert_eq!(combined(check, "lines", &results), Some(40.0));
    assert_eq!(combined(check, "uncovered", &results), Some(2.0));
    let scores = [
        support::score(1.0, "line_ratio", 30),
        support::score(0.5, "line_ratio", 10),
    ];
    let score = check.combine(&scores).unwrap();
    assert_eq!((score.value, score.n), (0.875, 40));
}
