// ABOUTME: tests.assertion-density over hand-built TestFrames, Symbols, and TestConventions: sparse tests,
// ABOUTME: sparse files against the repository, assertions per tested public function, scope, settings (ADR-104).
mod support;

use std::collections::BTreeSet;

use cupel_core::syntax::{
    Assertion, AssertionKind, AssertionStrength, Span, SymbolDef, SymbolKind, Symbols, TestCase,
    TestConventions, TestFile, TestFrames, TestPairing, Visibility,
};
use cupel_core::{
    ActionKind, CheckId, CheckResult, CheckStatus, CostClass, FactKind, Facts, Finding, FindingId,
    Location, MetricPer, Plan, Scope, Score, Severity,
};
use serde_json::{Value, json};

const ID: &str = "tests.assertion-density";

/// `lines` lines from `line`.
fn span(line: u32, lines: u32) -> Span {
    Span {
        line,
        col: 1,
        end_line: line + lines - 1,
        end_col: 2,
    }
}

/// A test case `lines` long at `line` making `assertions` assertions.
fn case(name: &str, line: u32, lines: u32, assertions: usize) -> TestCase {
    let at = span(line, lines);
    TestCase {
        name: name.to_owned(),
        span: at,
        assertions: (0..assertions)
            .map(|_| Assertion {
                kind: AssertionKind::Equality,
                strength: AssertionStrength::Strong,
                span: at,
            })
            .collect(),
        mocks: Vec::new(),
        skip: None,
        property: None,
    }
}

/// A test file of `cases`, each `lines` long, ten lines apart, making the given assertions.
fn file(path: &str, lines: u32, assertions: &[usize]) -> TestFile {
    TestFile {
        file: path.to_owned(),
        framework: "x-test".to_owned(),
        setup: Vec::new(),
        cases: assertions
            .iter()
            .zip(1u32..)
            .map(|(made, n)| case(&format!("case_{n}"), n * 40, lines, *made))
            .collect(),
    }
}

fn function(file: &str, name: &str, line: u32, visibility: Visibility) -> SymbolDef {
    SymbolDef {
        name: name.to_owned(),
        namespace: "m".to_owned(),
        display: format!("m::{name}"),
        kind: SymbolKind::Function,
        visibility,
        lang: cupel_core::LangId::from("x"),
        file: file.to_owned(),
        span: span(line, 3),
        body: None,
        parameters: 1,
    }
}

/// The facts and scope a run sees: test files, source symbols, and which tests pair with which
/// sources.
struct World {
    files: Vec<TestFile>,
    symbols: Vec<SymbolDef>,
    pairs: Vec<(&'static str, Vec<&'static str>)>,
}

impl World {
    fn of(files: Vec<TestFile>) -> Self {
        Self {
            files,
            symbols: Vec::new(),
            pairs: Vec::new(),
        }
    }

    fn with_symbols(mut self, symbols: Vec<SymbolDef>) -> Self {
        self.symbols = symbols;
        self
    }

    fn pairing(mut self, source: &'static str, tests: &[&'static str]) -> Self {
        self.pairs.push((source, tests.to_vec()));
        self
    }

    fn facts(&self) -> Facts {
        let mut facts = Facts::for_manifest(support::check(ID).manifest());
        facts
            .insert(TestFrames {
                files: self.files.clone(),
            })
            .unwrap();
        facts
            .insert(Symbols {
                symbols: self.symbols.clone(),
            })
            .unwrap();
        facts
            .insert(TestConventions {
                test_globs: vec!["t/**".to_owned()],
                pairs: self
                    .pairs
                    .iter()
                    .map(|(source, tests)| TestPairing {
                        source: (*source).to_owned(),
                        tests: tests.iter().map(|test| (*test).to_owned()).collect(),
                    })
                    .collect(),
            })
            .unwrap();
        facts
    }

    /// Every file the world mentions.
    fn scope(&self) -> Scope {
        let files: BTreeSet<String> = self
            .files
            .iter()
            .map(|file| file.file.clone())
            .chain(self.symbols.iter().map(|symbol| symbol.file.clone()))
            .collect();
        Scope::of_files(files.into_iter().collect())
    }

    fn run(&self) -> (CheckResult, Vec<Finding>) {
        self.run_in(self.scope(), json!({}))
    }

    fn run_in(&self, scope: Scope, config: Value) -> (CheckResult, Vec<Finding>) {
        let (result, recorded) =
            support::recorded(support::check(ID), &self.facts(), scope, config).unwrap();
        (result, recorded.findings)
    }
}

/// Each finding as (kind, location file, location line).
fn reported(findings: &[Finding]) -> Vec<(&str, &str, u32)> {
    findings
        .iter()
        .map(|finding| {
            let at = finding.location.as_ref().unwrap();
            (finding.kind.as_str(), at.file.as_str(), at.line)
        })
        .collect()
}

#[test]
fn the_manifest_needs_test_frames_symbols_and_conventions_at_tier_two_and_quick_cost() {
    let manifest = support::check(ID).manifest();
    assert_eq!(
        support::Shape::of(manifest),
        support::Shape {
            needs: vec![
                FactKind::TestFrames,
                FactKind::Symbols,
                FactKind::TestConventions
            ],
            wants: vec![],
            tier: 2,
            cost: CostClass::Quick,
            kinds: vec![
                ("density.sparse-test", ActionKind::StrengthenAssertion),
                ("density.sparse-file", ActionKind::StrengthenAssertion),
            ],
            metrics: vec![
                ("assertions_per_test", MetricPer::Scope),
                ("assertions_per_public_function", MetricPer::Scope),
                ("tested_public_functions", MetricPer::Scope),
                ("sparse_tests", MetricPer::Scope),
                ("sparse_files", MetricPer::Scope),
            ],
            evidence: vec!["object"; 2],
        }
    );
}

#[test]
fn a_long_test_with_few_assertions_is_sparse_and_its_finding_says_why() {
    let world = World::of(vec![TestFile {
        file: "t/a.x".to_owned(),
        framework: "x-test".to_owned(),
        setup: Vec::new(),
        cases: vec![
            case("one_in_thirty", 10, 30, 1),
            case("two_in_thirty", 50, 30, 2),
            case("one_in_twenty", 90, 20, 1),
            // A test that asserts nothing is tests.quality's `test.no-assertion`.
            case("none_in_thirty", 120, 30, 0),
        ],
    }]);
    let (result, findings) = world.run();
    assert_eq!(
        reported(&findings),
        [("density.sparse-test", "t/a.x", 10)],
        "{findings:#?}"
    );
    let finding = &findings[0];
    assert_eq!(
        finding.evidence.0,
        *json!({
            "test": "one_in_thirty",
            "assertions": 1,
            "lines": 30,
            "lines_per_assertion": 30,
            "lines_per_assertion_max": 20,
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(
        finding.id,
        FindingId::for_subject(
            &CheckId::from(ID),
            "density.sparse-test",
            "t/a.x::one_in_thirty"
        )
    );
    assert_eq!(
        finding.location,
        Some(Location {
            file: "t/a.x".to_owned(),
            line: 10,
            col: Some(1),
            end_line: 39,
            end_col: Some(2),
        })
    );
    assert_eq!(finding.severity, Severity::Warn);
    assert_eq!(finding.symbol, None);
    assert_eq!(
        finding.title,
        "`one_in_thirty` makes 1 assertion(s) in 30 lines (max 20 lines per assertion)"
    );
    assert_eq!(finding.action.kind, ActionKind::StrengthenAssertion);
    assert_eq!(
        finding.repro,
        "cupel check --only tests.assertion-density --paths t/a.x"
    );
    assert_eq!(
        finding.suppress,
        format!("cupel suppress {} --reason '…'", finding.id)
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.findings, 1);
    assert_eq!(result.metrics.get("sparse_tests"), Some(&1.0));
}

#[test]
fn the_lines_per_assertion_limit_is_a_setting() {
    let world = World::of(vec![file("t/a.x", 12, &[1, 3])]);
    let (_, strict) = world.run_in(world.scope(), json!({"lines_per_assertion_max": 10}));
    assert_eq!(reported(&strict), [("density.sparse-test", "t/a.x", 40)]);
    let (_, lax) = world.run_in(world.scope(), json!({"lines_per_assertion_max": 12}));
    assert_eq!(reported(&lax), []);
}

#[test]
fn a_file_far_below_the_repository_density_is_sparse() {
    let world = World::of(vec![
        file("t/dense.x", 5, &[4, 4, 4]),
        // One assertion per test against the repository's 15 / 6 = 2.5: 40 percent of it.
        file("t/sparse.x", 5, &[1, 1, 1]),
    ]);
    let (result, findings) = world.run();
    assert_eq!(
        reported(&findings),
        [("density.sparse-file", "t/sparse.x", 40)]
    );
    let finding = &findings[0];
    assert_eq!(
        finding.evidence.0,
        *json!({
            "tests": 3,
            "assertions": 3,
            "repository_tests": 6,
            "repository_assertions": 15,
            "percent": 40,
            "file_density_percent_min": 50,
            "file_tests_min": 3,
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(
        finding.id,
        FindingId::for_subject(&CheckId::from(ID), "density.sparse-file", "t/sparse.x")
    );
    assert_eq!(
        finding.location,
        Some(Location {
            file: "t/sparse.x".to_owned(),
            line: 40,
            col: Some(1),
            end_line: 124,
            end_col: Some(2),
        })
    );
    assert_eq!(
        finding.title,
        "`t/sparse.x` makes 1 assertion(s) per test, 40% of the repository's 2.5 (min 50%)"
    );
    assert_eq!(result.metrics.get("sparse_files"), Some(&1.0));
}

#[test]
fn a_file_with_few_tests_or_at_the_limit_is_not_sparse() {
    // 2 / 4 = 50 percent: at the limit, not below it.
    let at_limit = World::of(vec![
        file("t/dense.x", 5, &[3, 3, 3]),
        file("t/half.x", 5, &[1, 1, 1]),
    ]);
    assert_eq!(reported(&at_limit.run().1), []);
    let few = World::of(vec![
        file("t/dense.x", 5, &[4, 4, 4]),
        file("t/few.x", 5, &[1, 1]),
    ]);
    assert_eq!(reported(&few.run().1), []);
    let (_, configured) = few.run_in(
        few.scope(),
        json!({"file_tests_min": 2, "file_density_percent_min": 60}),
    );
    assert_eq!(
        reported(&configured),
        [("density.sparse-file", "t/few.x", 40)]
    );
}

#[test]
fn assertions_per_tested_public_function_count_public_functions_whose_tests_ran() {
    let world = World::of(vec![
        file("t/a.x", 5, &[2, 4]),
        file("t/b.x", 5, &[3]),
        file("t/c.x", 5, &[1]),
    ])
    .with_symbols(vec![
        function("src/a.x", "parse", 1, Visibility::Public),
        function("src/a.x", "render", 10, Visibility::Public),
        function("src/a.x", "helper", 20, Visibility::Private),
        function("src/b.x", "load", 1, Visibility::Public),
        // No test file pairs with src/untested.x.
        function("src/untested.x", "lonely", 1, Visibility::Public),
        // A function in a test file is not code under test.
        function("t/c.x", "fixture", 1, Visibility::Public),
    ])
    .pairing("src/a.x", &["t/a.x", "t/c.x"])
    .pairing("src/b.x", &["t/b.x"]);
    let (result, findings) = world.run();
    assert_eq!(reported(&findings), []);
    // src/a.x: (2 + 4 + 1) assertions over 2 public functions; src/b.x: 3 over 1.
    assert_eq!(
        result.metrics.get("assertions_per_public_function"),
        Some(&(10.0 / 3.0))
    );
    assert_eq!(result.metrics.get("tested_public_functions"), Some(&3.0));
    assert_eq!(
        result.metrics.get("assertions_per_test"),
        Some(&(10.0 / 4.0))
    );
    assert_eq!(
        result.score,
        Some(Score {
            value: 2.5,
            unit: "assertions_per_test".to_owned(),
            n: 4,
        })
    );
    assert_eq!(result.status, CheckStatus::Pass);
}

#[test]
fn findings_and_metrics_cover_the_scope_and_the_repository_is_every_test_seen() {
    let world = World::of(vec![
        file("t/dense.x", 30, &[4, 4, 4]),
        file("t/sparse.x", 30, &[1, 1, 1]),
    ])
    .with_symbols(vec![
        function("src/a.x", "parse", 1, Visibility::Public),
        function("src/b.x", "load", 1, Visibility::Public),
    ])
    .pairing("src/a.x", &["t/dense.x"])
    .pairing("src/b.x", &["t/sparse.x"]);
    let scope = Scope::of_files(vec!["t/sparse.x".to_owned(), "src/b.x".to_owned()]);
    let (result, findings) = world.run_in(scope, json!({}));
    assert_eq!(
        reported(&findings),
        [
            ("density.sparse-test", "t/sparse.x", 40),
            ("density.sparse-test", "t/sparse.x", 80),
            ("density.sparse-test", "t/sparse.x", 120),
            ("density.sparse-file", "t/sparse.x", 40),
        ]
    );
    assert_eq!(result.metrics.get("assertions_per_test"), Some(&1.0));
    assert_eq!(
        result.metrics.get("assertions_per_public_function"),
        Some(&3.0)
    );
    assert_eq!(result.metrics.get("tested_public_functions"), Some(&1.0));
    let symbols = Scope {
        files: vec!["t/sparse.x".to_owned()],
        symbols: Some(BTreeSet::from([cupel_core::syntax::SymbolKey {
            file: "t/sparse.x".to_owned(),
            line: 80,
            col: 1,
        }])),
    };
    let (narrow, findings) = world.run_in(symbols, json!({}));
    assert_eq!(
        reported(&findings),
        [
            ("density.sparse-test", "t/sparse.x", 80),
            ("density.sparse-file", "t/sparse.x", 40),
        ]
    );
    assert_eq!(narrow.score.unwrap().n, 1);
}

#[test]
fn no_tests_pass_with_zero_density() {
    let (result, findings) = World::of(Vec::new()).run();
    assert!(findings.is_empty());
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(
        result.metrics,
        [
            ("assertions_per_public_function", 0.0),
            ("assertions_per_test", 0.0),
            ("sparse_files", 0.0),
            ("sparse_tests", 0.0),
            ("tested_public_functions", 0.0),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
    );
}

#[test]
fn unknown_settings_and_bad_values_are_configuration_errors() {
    let check = support::check(ID);
    for (config, key) in [
        (json!({"lines_per_test_max": 3}), "lines_per_test_max"),
        (
            json!({"lines_per_assertion_max": -1}),
            "lines_per_assertion_max",
        ),
        (json!({"file_tests_min": "few"}), "file_tests_min"),
    ] {
        let (named, valid) = support::setting_error(check, config);
        assert_eq!(named, key);
        assert_eq!(
            valid,
            [
                "lines_per_assertion_max",
                "file_density_percent_min",
                "file_tests_min"
            ]
        );
    }
}

#[test]
fn plans_count_the_files_in_scope_at_quick_cost() {
    let check = support::check(ID);
    let (ctx, _dir) = support::ctx(Scope::default(), json!({}));
    let scope = Scope::of_files(vec!["t/a.x".to_owned(), "t/b.x".to_owned()]);
    assert_eq!(
        check.plan(&ctx, &scope).unwrap(),
        Plan::estimate(CostClass::Quick, 2)
    );
}

#[test]
fn densities_combine_across_languages_weighted_by_what_they_divide_by() {
    let check = support::check(ID);
    let results = [
        support::language(
            check,
            &[
                ("assertions_per_test", 1.0),
                ("assertions_per_public_function", 2.0),
                ("tested_public_functions", 1.0),
                ("sparse_tests", 2.0),
            ],
            1,
        ),
        support::language(
            check,
            &[
                ("assertions_per_test", 4.0),
                ("assertions_per_public_function", 5.0),
                ("tested_public_functions", 2.0),
                ("sparse_tests", 1.0),
            ],
            3,
        ),
    ];
    let combined = |name| support::combined(check, name, &results);
    assert_eq!(combined("assertions_per_test"), Some((1.0 + 12.0) / 4.0));
    assert_eq!(
        combined("assertions_per_public_function"),
        Some((2.0 + 10.0) / 3.0)
    );
    assert_eq!(combined("tested_public_functions"), Some(3.0));
    assert_eq!(combined("sparse_tests"), Some(3.0));
}
