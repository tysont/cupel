// ABOUTME: tests.quality over hand-built TestFrames, Symbols, NormTree, and Decisions: each finding kind,
// ABOUTME: its thresholds and SCRAP evidence, the Throws rule, scope, identity, and the score (ADR-054/055).
mod support;

use std::collections::BTreeSet;

use cupel_core::syntax::{
    Assertion, AssertionKind, AssertionStrength, Decision, DecisionKind, Decisions, Mock, NormNode,
    NormTree, NormalizedSymbol, SkipMarker, Span, SymbolDecisions, SymbolDef, SymbolKind, Symbols,
    TestCase, TestFile, TestFrames, Visibility,
};
use cupel_core::{
    ActionKind, CheckId, CheckResult, CheckStatus, FactKind, Facts, Finding, FindingId, Manifest,
    MetricPer, Plan, Scope, Score, Severity, SymbolHash,
};
use serde_json::{Value, json};
use support::{Recorded, combined, language, pairs, span};

const ID: &str = "tests.quality";

/// A statement whose structure is named by `shape`: equal shapes are equal statements.
fn statement(shape: &str) -> NormNode {
    NormNode::new(
        "statement",
        vec![NormNode::new(shape, vec![NormNode::leaf("$id")])],
    )
}

fn statements(shapes: &[&str]) -> Vec<NormNode> {
    shapes.iter().map(|shape| statement(shape)).collect()
}

fn hash_of(file: &str, line: u32) -> SymbolHash {
    let file_bits = u64::from(file.bytes().map(u32::from).sum::<u32>());
    format!("sh_{:016x}", (file_bits << 32) | u64::from(line))
        .parse()
        .unwrap()
}

/// One test case and the facts about its symbol.
#[derive(Clone)]
struct Case {
    file: &'static str,
    name: &'static str,
    line: u32,
    assertions: Vec<(AssertionKind, AssertionStrength)>,
    mocks: u32,
    skipped: bool,
    decisions: Vec<DecisionKind>,
    /// Loops over tables of cases written in place, after `decisions`.
    tables: usize,
    body: Vec<NormNode>,
    /// The body's statements sit inside one list node, as Go's do.
    wrapped: bool,
}

fn case(name: &'static str, line: u32) -> Case {
    Case {
        file: "t.x",
        name,
        line,
        assertions: vec![(AssertionKind::Equality, AssertionStrength::Strong)],
        mocks: 0,
        skipped: false,
        decisions: Vec::new(),
        tables: 0,
        body: Vec::new(),
        wrapped: false,
    }
}

impl Case {
    fn asserting(mut self, assertions: &[(AssertionKind, AssertionStrength)]) -> Self {
        self.assertions = assertions.to_vec();
        self
    }

    fn mocking(mut self, mocks: u32) -> Self {
        self.mocks = mocks;
        self
    }

    fn skipped(mut self) -> Self {
        self.skipped = true;
        self
    }

    fn deciding(mut self, decisions: &[DecisionKind]) -> Self {
        self.decisions = decisions.to_vec();
        self
    }

    fn iterating_tables(mut self, tables: usize) -> Self {
        self.tables = tables;
        self
    }

    fn with_body(mut self, body: Vec<NormNode>) -> Self {
        self.body = body;
        self
    }

    fn wrapped(mut self) -> Self {
        self.wrapped = true;
        self
    }

    fn in_file(mut self, file: &'static str) -> Self {
        self.file = file;
        self
    }

    fn span(&self) -> Span {
        span(self.line, 3)
    }

    fn symbol(&self) -> SymbolDef {
        SymbolDef {
            name: self.name.to_owned(),
            namespace: "t".to_owned(),
            display: format!("t::{}", self.name),
            kind: SymbolKind::Function,
            visibility: Visibility::Private,
            lang: cupel_core::LangId::from("x"),
            file: self.file.to_owned(),
            span: self.span(),
            body: None,
            parameters: 0,
        }
    }

    fn frame(&self) -> TestCase {
        let at = self.span();
        TestCase {
            name: self.name.to_owned(),
            span: at,
            assertions: self
                .assertions
                .iter()
                .map(|(kind, strength)| Assertion {
                    kind: *kind,
                    strength: *strength,
                    span: at,
                })
                .collect(),
            mocks: (0..self.mocks)
                .map(|n| Mock {
                    target: format!("double{n}"),
                    span: at,
                })
                .collect(),
            skip: self.skipped.then_some(SkipMarker {
                reason: None,
                span: at,
            }),
            property: None,
            doc: None,
        }
    }

    fn tree(&self) -> NormNode {
        let body = if self.wrapped {
            NormNode::new("body", vec![NormNode::new("list", self.body.clone())])
        } else {
            NormNode::new("body", self.body.clone())
        };
        NormNode::new(
            "function",
            vec![NormNode::new("parameters", Vec::new()), body],
        )
    }
}

struct World {
    cases: Vec<Case>,
}

impl World {
    fn of(cases: Vec<Case>) -> Self {
        Self { cases }
    }

    fn frames(&self) -> TestFrames {
        let mut files: Vec<TestFile> = Vec::new();
        for case in &self.cases {
            let frame = case.frame();
            match files.iter_mut().find(|file| file.file == case.file) {
                Some(file) => file.cases.push(frame),
                None => files.push(TestFile {
                    file: case.file.to_owned(),
                    framework: "x-test".to_owned(),
                    setup: Vec::new(),
                    cases: vec![frame],
                }),
            }
        }
        TestFrames { files }
    }

    /// The facts the manifest declares; `Decisions` only when `with_decisions`.
    fn facts(&self, manifest: &Manifest, with_decisions: bool) -> Facts {
        let mut facts = Facts::for_manifest(manifest);
        facts.insert(self.frames()).unwrap();
        facts
            .insert(Symbols {
                symbols: self.cases.iter().map(Case::symbol).collect(),
            })
            .unwrap();
        facts
            .insert(NormTree {
                symbols: self
                    .cases
                    .iter()
                    .map(|case| NormalizedSymbol {
                        symbol: case.symbol().key(),
                        hash: hash_of(case.file, case.line),
                        root: case.tree(),
                    })
                    .collect(),
            })
            .unwrap();
        if with_decisions {
            facts
                .insert(Decisions {
                    symbols: self
                        .cases
                        .iter()
                        .map(|case| SymbolDecisions {
                            symbol: case.symbol().key(),
                            decisions: case
                                .decisions
                                .iter()
                                .map(|kind| (*kind, false))
                                .chain((0..case.tables).map(|_| (DecisionKind::Loop, true)))
                                .map(|(kind, table)| Decision {
                                    kind,
                                    nesting: 0,
                                    cyclomatic: 1,
                                    cognitive: 1,
                                    table,
                                    continues: false,
                                })
                                .collect(),
                        })
                        .collect(),
                })
                .unwrap();
        }
        facts
    }

    fn scope(&self) -> Scope {
        let files: BTreeSet<String> = self.cases.iter().map(|c| c.file.to_owned()).collect();
        Scope {
            files: files.into_iter().collect(),
            symbols: Some(self.cases.iter().map(|c| c.symbol().key()).collect()),
        }
    }
}

struct Run {
    world: World,
    scope: Option<Scope>,
    config: Value,
    decisions: bool,
}

impl Run {
    fn of(cases: Vec<Case>) -> Self {
        Self {
            world: World::of(cases),
            scope: None,
            config: json!({}),
            decisions: true,
        }
    }

    fn config(mut self, config: Value) -> Self {
        self.config = config;
        self
    }

    fn scope(mut self, scope: Scope) -> Self {
        self.scope = Some(scope);
        self
    }

    fn without_decisions(mut self) -> Self {
        self.decisions = false;
        self
    }

    fn go(self) -> (CheckResult, Vec<Finding>) {
        let (result, recorded) = self.recorded();
        (result, recorded.findings)
    }

    /// The result and every finding and measurement the check reported.
    fn recorded(self) -> (CheckResult, Recorded) {
        let check = support::check(ID);
        let scope = self.scope.unwrap_or_else(|| self.world.scope());
        let facts = self.world.facts(check.manifest(), self.decisions);
        support::recorded(check, &facts, scope, self.config).unwrap()
    }
}

use AssertionKind::{Custom, Equality, NotNull, Throws, Truthiness};
use AssertionStrength::{Strong, Weak};

/// How each finding kind's evidence schema falls short: it must inline its enums, require
/// `remediation_mode` and `actionability`, and spell out the actionability levels.
fn evidence_problems(manifest: &cupel_core::Manifest) -> Vec<String> {
    let mut problems = Vec::new();
    for spec in &manifest.findings {
        let schema = Value::Object(spec.evidence.clone());
        if schema.to_string().contains("$ref") {
            problems.push(format!("{} refers to a definition", spec.kind));
        }
        let required = schema["required"].as_array().cloned().unwrap_or_default();
        for field in ["remediation_mode", "actionability"] {
            if !required.contains(&json!(field)) {
                problems.push(format!("{} does not require {field}", spec.kind));
            }
        }
        if schema["properties"]["actionability"]["enum"]
            != json!(["mechanical", "guided", "judgment"])
        {
            problems.push(format!("{} misspells actionability", spec.kind));
        }
    }
    problems
}

#[test]
fn the_manifest_declares_its_facts_kinds_actions_and_metrics() {
    let manifest = support::check(ID).manifest();
    assert_eq!(
        support::Shape::of(manifest),
        support::Shape {
            needs: vec![FactKind::TestFrames, FactKind::Symbols, FactKind::NormTree],
            wants: vec![FactKind::Decisions],
            tier: 2,
            cost: cupel_core::CostClass::Quick,
            kinds: vec![
                ("test.weak-assertion", ActionKind::StrengthenAssertion),
                ("test.no-assertion", ActionKind::WriteTest),
                ("test.mock-heavy", ActionKind::Review),
                ("test.logic", ActionKind::RefactorExtract),
                ("test.duplicate-setup", ActionKind::RefactorExtract)
            ],
            metrics: vec![
                ("assertion_density", MetricPer::Scope),
                ("weak_ratio", MetricPer::Scope),
                ("no_assertion", MetricPer::Scope),
                ("mock_density", MetricPer::Symbol)
            ],
            evidence: vec!["object"; 5],
        }
    );
    assert_eq!(evidence_problems(manifest), Vec::<String>::new());
}

#[test]
fn a_test_whose_every_assertion_is_weak_is_reported_and_one_strong_assertion_suffices() {
    let (_, findings) = Run::of(vec![
        case("weak_only", 1).asserting(&[(Truthiness, Weak), (NotNull, Weak)]),
        case("mixed", 10).asserting(&[(Truthiness, Weak), (Equality, Strong)]),
        case("strong", 20),
    ])
    .go();
    assert_eq!(
        support::kinds(&findings),
        pairs(&[("t::weak_only", "test.weak-assertion")])
    );
    assert_eq!(
        findings[0].evidence.0,
        *json!({
            "remediation_mode": "strengthen",
            "actionability": "guided",
            "assertions": 2,
            "weak": 2,
            "kinds": ["not-null", "truthiness"],
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(findings[0].action.kind, ActionKind::StrengthenAssertion);
}

#[test]
fn a_test_without_assertions_is_reported_but_a_thrown_error_counts_as_one() {
    let (_, findings) = Run::of(vec![
        case("checks_nothing", 1).asserting(&[]),
        // `#[should_panic(expected = …)]`, `pytest.raises(E)`, `toThrow("message")`.
        case("panics_as_expected", 10).asserting(&[(Throws, Strong)]),
        case("custom_matcher", 20).asserting(&[(Custom, Strong)]),
    ])
    .go();
    assert_eq!(
        support::kinds(&findings),
        pairs(&[("t::checks_nothing", "test.no-assertion")])
    );
    assert_eq!(
        findings[0].evidence.0,
        *json!({
            "remediation_mode": "add-assertion",
            "actionability": "guided",
            "mocks": 0,
            "skipped": false,
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(findings[0].action.kind, ActionKind::WriteTest);
}

#[test]
fn a_bare_should_panic_is_a_weak_throws_assertion_not_a_missing_one() {
    let (_, findings) = Run::of(vec![case("panics_somehow", 1).asserting(&[(Throws, Weak)])]).go();
    assert_eq!(
        support::kinds(&findings),
        pairs(&[("t::panics_somehow", "test.weak-assertion")])
    );
}

#[test]
fn mock_heavy_needs_enough_doubles_and_a_density_above_the_limit() {
    let (_, findings) = Run::of(vec![
        // 2 / (2 + 1) = 66% > 50%.
        case("two_doubles_one_check", 1).mocking(2),
        // 2 / (2 + 2) = 50%: at the limit.
        case("two_doubles_two_checks", 10)
            .mocking(2)
            .asserting(&[(Equality, Strong), (Equality, Strong)]),
        // 100%, but one double is below mocks_min.
        case("one_double_no_check", 20).asserting(&[]).mocking(1),
    ])
    .go();
    assert_eq!(
        support::kinds(&findings),
        pairs(&[
            ("t::two_doubles_one_check", "test.mock-heavy"),
            ("t::one_double_no_check", "test.no-assertion"),
        ])
    );
    assert_eq!(
        findings[0].evidence.0,
        *json!({
            "remediation_mode": "decouple",
            "actionability": "judgment",
            "mocks": 2,
            "assertions": 1,
            "density": 66,
            "mock_density_max": 50,
            "mocks_min": 2,
            "targets": ["double0", "double1"],
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(findings[0].action.kind, ActionKind::Review);
}

#[test]
fn mock_thresholds_come_from_the_configuration() {
    let cases = || vec![case("two_doubles_one_check", 1).mocking(2)];
    let (_, loose) = Run::of(cases())
        .config(json!({"mock_density_max": 70}))
        .go();
    assert!(loose.is_empty(), "{loose:?}");
    let (_, fewer) = Run::of(cases()).config(json!({"mocks_min": 3})).go();
    assert!(fewer.is_empty(), "{fewer:?}");
}

#[test]
fn doubles_without_assertions_are_both_mock_heavy_and_assertion_free() {
    let (_, findings) = Run::of(vec![case("only_doubles", 1).asserting(&[]).mocking(3)]).go();
    assert_eq!(
        support::kinds(&findings),
        pairs(&[
            ("t::only_doubles", "test.no-assertion"),
            ("t::only_doubles", "test.mock-heavy"),
        ])
    );
}

#[test]
fn loops_and_unguarded_branches_are_logic_but_guarded_failure_calls_are_not() {
    use DecisionKind::{BooleanOp, Branch, Case as Switch, Catch, EarlyReturn, Loop, Ternary};
    let (_, findings) = Run::of(vec![
        case("loops", 1).deciding(&[Loop]),
        // `if got != want { t.Errorf(…) }`: the branch is the assertion.
        case("guarded_assertion", 10).deciding(&[Branch]),
        // `if testing.Short() { t.Skip() }` plus one guarded assertion.
        case("guarded_skip", 20)
            .skipped()
            .deciding(&[Branch, Branch]),
        case("branches_beyond_guards", 30).deciding(&[Branch, Branch]),
        case("alternatives", 40).deciding(&[Switch, Ternary, Catch]),
        case("operators_and_returns", 50).deciding(&[BooleanOp, EarlyReturn]),
        // `for (input, want) in [(1, 2), (3, 4)] { assert_eq!(…) }`: a table of cases.
        case("table_driven", 60).iterating_tables(2),
        case("table_with_logic", 70)
            .iterating_tables(1)
            .deciding(&[Loop]),
    ])
    .go();
    assert_eq!(
        support::kinds(&findings),
        pairs(&[
            ("t::loops", "test.logic"),
            ("t::branches_beyond_guards", "test.logic"),
            ("t::alternatives", "test.logic"),
            ("t::table_with_logic", "test.logic"),
        ])
    );
    assert_eq!(
        findings[1].evidence.0,
        *json!({
            "remediation_mode": "simplify",
            "actionability": "guided",
            "logic": 1,
            "logic_max": 0,
            "loops": 0,
            "tables": 0,
            "branches": 2,
            "alternatives": 0,
            "guards": 1,
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(findings[2].evidence.0["logic"], json!(3));
    assert_eq!(
        (
            &findings[3].evidence.0["logic"],
            &findings[3].evidence.0["loops"],
            &findings[3].evidence.0["tables"]
        ),
        (&json!(1), &json!(1), &json!(1)),
        "a table loop is not logic; the other loop is"
    );
    assert_eq!(findings[0].action.kind, ActionKind::RefactorExtract);
}

#[test]
fn logic_is_allowed_up_to_logic_max_and_skipped_without_decisions() {
    use DecisionKind::Loop;
    let cases = || vec![case("loops", 1).deciding(&[Loop])];
    let (_, allowed) = Run::of(cases()).config(json!({"logic_max": 1})).go();
    assert!(allowed.is_empty(), "{allowed:?}");
    let (result, without) = Run::of(cases()).without_decisions().go();
    assert!(without.is_empty(), "{without:?}");
    assert_eq!(result.status, CheckStatus::Pass);
}

#[test]
fn tests_repeating_their_leading_statements_are_one_duplicate_setup_finding() {
    let setup = ["open", "deposit", "deposit"];
    let with_tail = |tail: &[&str]| {
        let mut body = statements(&setup);
        body.extend(statements(tail));
        body
    };
    let (_, findings) = Run::of(vec![
        case("first", 1).with_body(with_tail(&["assert_statement"])),
        case("unrelated", 10).with_body(statements(&["open", "withdraw", "deposit", "check"])),
        case("second", 20).with_body(with_tail(&["let", "check"])),
        case("third", 30).with_body(with_tail(&["deposit", "check"])),
        // The whole body is the shared part: nothing follows it, so it is not setup.
        case("only_setup", 40).with_body(statements(&setup)),
        // The same statements in another file are not this file's duplication.
        case("elsewhere", 1)
            .in_file("u.x")
            .with_body(with_tail(&["check"])),
    ])
    .go();
    assert_eq!(
        support::kinds(&findings),
        pairs(&[("t::first", "test.duplicate-setup")])
    );
    assert_eq!(
        findings[0].evidence.0,
        *json!({
            "remediation_mode": "extract-fixture",
            "actionability": "mechanical",
            "statements": 3,
            "setup_statements_min": 3,
            "tests": ["t::first", "t::second", "t::third"],
            "lines": [1, 20, 30],
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(findings[0].action.kind, ActionKind::RefactorExtract);
}

#[test]
fn the_shared_prefix_counts_every_statement_all_members_repeat() {
    let body = |tail: &str| statements(&["a", "b", "c", "d", tail]);
    let (_, findings) = Run::of(vec![
        case("first", 1).with_body(body("x")),
        case("second", 10).with_body(body("y")),
    ])
    .go();
    assert_eq!(findings[0].evidence.0["statements"], json!(4));
}

#[test]
fn statements_wrapped_in_one_list_node_are_compared_and_leaf_lists_are_not_statements() {
    let setup = ["open", "deposit", "deposit", "check"];
    let declaration = |value: &str| {
        // `let mut x = call(…)`: one statement, whose parts include leaf tokens.
        vec![NormNode::new(
            "let",
            vec![
                NormNode::leaf("mut"),
                NormNode::leaf("$id"),
                NormNode::leaf("="),
                statement(value),
            ],
        )]
    };
    let (_, findings) = Run::of(vec![
        case("wrapped_one", 1)
            .wrapped()
            .with_body(statements(&setup)),
        case("wrapped_two", 10)
            .wrapped()
            .with_body(statements(&["open", "deposit", "deposit", "other"])),
        case("let_one", 20).with_body(declaration("first")),
        case("let_two", 30).with_body(declaration("second")),
    ])
    .go();
    assert_eq!(
        support::kinds(&findings),
        pairs(&[("t::wrapped_one", "test.duplicate-setup")])
    );
    let (_, longer) = Run::of(vec![
        case("one", 1).with_body(statements(&setup)),
        case("two", 10).with_body(statements(&["open", "deposit", "deposit", "other"])),
    ])
    .config(json!({"setup_statements_min": 4}))
    .go();
    assert!(longer.is_empty(), "{longer:?}");
}

#[test]
fn a_finding_is_identified_located_and_reproducible_by_its_test_symbol() {
    let (_, findings) = Run::of(vec![case("checks_nothing", 7).asserting(&[])]).go();
    let [finding] = findings.as_slice() else {
        panic!("one finding expected: {findings:?}")
    };
    let hash = hash_of("t.x", 7);
    let check = CheckId::from(ID);
    assert_eq!(
        finding.id,
        FindingId::compute(&check, "test.no-assertion", &hash, None)
    );
    assert_eq!(finding.severity, Severity::Warn);
    assert_eq!(finding.tier.value(), 2);
    assert_eq!(finding.symbol.as_ref().unwrap().hash, hash);
    let location = finding.location.as_ref().unwrap();
    assert_eq!(
        (location.file.as_str(), location.line, location.end_line),
        ("t.x", 7, 9)
    );
    assert_eq!(
        finding.repro,
        format!("cupel check --only tests.quality --symbols {hash}")
    );
}

#[test]
fn only_tests_in_scope_are_measured() {
    let cases = vec![
        case("inside", 1).asserting(&[]),
        case("outside", 10).asserting(&[]),
    ];
    let inside = cases[0].symbol().key();
    let (result, findings) = Run::of(cases)
        .scope(Scope {
            files: vec!["t.x".to_owned()],
            symbols: Some(BTreeSet::from([inside])),
        })
        .go();
    assert_eq!(
        support::kinds(&findings),
        pairs(&[("t::inside", "test.no-assertion")])
    );
    assert_eq!(result.score.unwrap().n, 1);
}

#[test]
fn the_score_is_assertions_per_test_and_clean_tests_pass() {
    let (result, findings) = Run::of(vec![
        case("one", 1),
        case("three", 10).asserting(&[(Equality, Strong), (Truthiness, Weak), (Throws, Strong)]),
    ])
    .go();
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.findings, 0);
    assert_eq!(
        result.score,
        Some(Score {
            value: 2.0,
            unit: "assertion_density".to_owned(),
            n: 2,
        })
    );
    let (failing, _) = Run::of(vec![case("none", 1).asserting(&[])]).go();
    assert_eq!(failing.status, CheckStatus::Fail);
    assert_eq!(failing.findings, 1);
}

#[test]
fn unknown_settings_and_bad_values_are_configuration_errors() {
    let check = support::check(ID);
    for (config, key) in [
        (json!({"mock_ratio": 3}), "mock_ratio"),
        (json!({"mock_density_max": -1}), "mock_density_max"),
        (json!({"logic_max": "none"}), "logic_max"),
        (json!({"setup_statements_min": 2.5}), "setup_statements_min"),
    ] {
        let (named, valid) = support::setting_error(check, config);
        assert_eq!(named, key);
        assert_eq!(
            valid,
            [
                "mock_density_max",
                "mocks_min",
                "logic_max",
                "setup_statements_min"
            ]
        );
    }
}

#[test]
fn plans_count_the_symbols_in_scope_at_quick_cost() {
    let check = support::check(ID);
    let scope = World::of(vec![case("a", 1), case("b", 10)]).scope();
    let (ctx, _dir) = support::ctx(scope.clone(), json!({}));
    assert_eq!(
        check.plan(&ctx, &scope).unwrap(),
        Plan {
            targets: 2,
            est_ms: 2
        }
    );
}

/// Four tests: five assertions, one of them weak; one test asserts nothing; one has three doubles
/// for its one assertion.
fn four_tests() -> Vec<Case> {
    vec![
        case("one", 1),
        case("three", 10).asserting(&[(Equality, Strong), (Truthiness, Weak), (Throws, Strong)]),
        case("none", 20).asserting(&[]),
        case("mocky", 30).mocking(3),
    ]
}

#[test]
fn the_result_reports_density_weak_share_and_tests_without_assertions() {
    let (result, _) = Run::of(four_tests()).go();
    assert_eq!(
        result.metrics,
        [
            ("assertion_density".to_owned(), 1.25),
            ("no_assertion".to_owned(), 1.0),
            ("weak_ratio".to_owned(), 0.2),
        ]
        .into()
    );
    let (empty, _) = Run::of(vec![]).go();
    assert!(
        empty.metrics.values().all(|value| *value == 0.0),
        "{:?}",
        empty.metrics
    );
}

#[test]
fn each_test_in_scope_is_measured_by_its_mock_density() {
    let (_, recorded) = Run::of(four_tests()).recorded();
    assert_eq!(
        recorded.measured(),
        [
            ("mock_density", "t::one", 0.0),
            ("mock_density", "t::three", 0.0),
            ("mock_density", "t::none", 0.0),
            ("mock_density", "t::mocky", 75.0),
        ]
    );
    let mocky = &recorded.measurements[3].target;
    assert_eq!(mocky.symbol, Some(hash_of("t.x", 30)));
}

#[test]
fn ratios_combine_over_every_languages_tests_and_counts_add_up() {
    let check = support::check(ID);
    let results = [
        language(
            check,
            &[
                ("assertion_density", 2.0),
                ("weak_ratio", 0.5),
                ("no_assertion", 1.0),
            ],
            2,
        ),
        language(
            check,
            &[
                ("assertion_density", 1.0),
                ("weak_ratio", 0.0),
                ("no_assertion", 2.0),
            ],
            6,
        ),
    ];
    assert_eq!(
        combined(check, "assertion_density", &results),
        Some(1.25),
        "10 assertions over 8 tests"
    );
    assert_eq!(
        combined(check, "weak_ratio", &results),
        Some(0.2),
        "2 weak of 10 assertions"
    );
    assert_eq!(combined(check, "no_assertion", &results), Some(3.0));
    let empty = [language(
        check,
        &[("assertion_density", 0.0), ("weak_ratio", 0.0)],
        0,
    )];
    assert_eq!(combined(check, "weak_ratio", &empty), Some(0.0));
}
