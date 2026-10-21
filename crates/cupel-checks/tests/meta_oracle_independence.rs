// ABOUTME: meta.oracle-independence over synthetic facts: a changed function exercised only by tests the range added is
// ABOUTME: self-authored evidence; one a pre-existing test runs is independent; without a per-test map nothing is labeled.
mod support;

use cupel_core::syntax::{
    NormNode, NormTree, NormalizedSymbol, Span, SymbolDef, SymbolKind, Symbols, TestCase, TestFile,
    TestFrames, Visibility,
};
use cupel_core::{
    ActionKind, CheckResult, CheckStatus, CostClass, CoveredLines, FactKind, Facts, Finding,
    LangId, MetricPer, PerTestMap, Produced, ProducerId, Range, Scope, Severity, TestCoverage,
};
use serde_json::json;
use support::{Shape, hash_of_line, score};

const ID: &str = "meta.oracle-independence";

/// The lines from `line` to `end_line`.
fn lines(line: u32, end_line: u32) -> Span {
    support::span(line, end_line - line + 1)
}

fn symbol(name: &str, line: u32, end_line: u32) -> SymbolDef {
    SymbolDef {
        name: name.to_owned(),
        namespace: String::new(),
        display: name.to_owned(),
        kind: SymbolKind::Function,
        visibility: Visibility::Public,
        lang: LangId::from("python"),
        file: if name.starts_with("test_") {
            "tests/test_dates.py".to_owned()
        } else {
            "src/dates.py".to_owned()
        },
        span: lines(line, end_line),
        body: None,
        parameters: 1,
    }
}

fn case(name: &str, line: u32) -> TestCase {
    TestCase {
        name: name.to_owned(),
        span: lines(line, line + 2),
        assertions: Vec::new(),
        mocks: Vec::new(),
        skip: None,
        property: None,
        doc: None,
    }
}

fn frames(cases: Vec<TestCase>) -> TestFrames {
    TestFrames {
        files: vec![TestFile {
            file: "tests/test_dates.py".to_owned(),
            framework: "pytest".to_owned(),
            setup: Vec::new(),
            cases,
        }],
    }
}

/// `test` (a node ID in `tests/test_dates.py`) ran `lines` of `src/dates.py`.
fn ran(test: &str, lines: &[u32]) -> TestCoverage {
    TestCoverage::new(
        &format!("tests/test_dates.py::{test}"),
        Some("tests/test_dates.py".to_owned()),
        vec![CoveredLines {
            file: "src/dates.py".to_owned(),
            lines: lines.to_vec(),
        }],
    )
}

/// `parse` (lines 1–5) and `format` (lines 7–9) changed; the head's tests are
/// `test_parses_iso` (also at the base) and `test_formats` (added by the range).
fn labeled(map: Option<Vec<TestCoverage>>) -> (CheckResult, Vec<Finding>) {
    let check = support::check(ID);
    let symbols = vec![
        symbol("parse", 1, 5),
        symbol("format", 7, 9),
        symbol("test_parses_iso", 1, 3),
        symbol("test_formats", 5, 7),
    ];
    let mut facts = Facts::for_manifest(check.manifest());
    facts
        .insert(NormTree {
            symbols: symbols
                .iter()
                .map(|symbol| NormalizedSymbol {
                    symbol: symbol.key(),
                    hash: hash_of_line(symbol.span.line * 100 + symbol.span.end_line),
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
    facts.insert(Symbols { symbols }).unwrap();
    facts
        .insert(frames(vec![
            case("test_parses_iso", 1),
            case("test_formats", 5),
        ]))
        .unwrap();
    facts
        .insert_base_shared(&Produced::new(frames(vec![case("test_parses_iso", 1)])).share())
        .unwrap();
    facts.set_range(Range {
        commit: "0a1b2c3".to_owned(),
        changes: Vec::new(),
    });
    if let Some(tests) = map {
        facts
            .insert(PerTestMap::new(ProducerId::from("tool.pytest"), tests))
            .unwrap();
    }
    let scope = Scope::of_files(vec![
        "src/dates.py".to_owned(),
        "tests/test_dates.py".to_owned(),
    ]);
    let (result, recorded) = support::recorded(check, &facts, scope, json!({})).unwrap();
    (result, recorded.findings)
}

#[test]
fn a_changed_function_only_tests_the_range_added_exercise_is_self_authored_evidence() {
    let (result, findings) = labeled(Some(vec![
        ran("test_parses_iso", &[2, 3]),
        ran("test_formats", &[2, 8]),
    ]));
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(
        (finding.kind.as_str(), finding.severity, finding.action.kind),
        (
            "evidence.self-authored-only",
            Severity::Warn,
            ActionKind::Review
        )
    );
    assert_eq!(finding.symbol.as_ref().unwrap().display, "format");
    assert_eq!(
        finding.title,
        "`format` is exercised only by tests the change added (1), none that existed before it"
    );
    assert_eq!(
        serde_json::to_value(&finding.evidence).unwrap(),
        json!({"base": "0a1b2c3", "tests": ["tests/test_dates.py::test_formats"]})
    );
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(
        result.metrics,
        [
            ("independent".to_owned(), 1.0),
            ("self_authored_only".to_owned(), 1.0)
        ]
        .into()
    );
    assert_eq!(
        result.score,
        Some(score(0.5, "independently tested share", 2))
    );
    assert_eq!(
        result.notes,
        [
            "tests that existed before the change exercise 1 of the 2 changed functions tests \
          run; an oracle's independence is its verification (`cupel oracle verify`)"
        ]
    );
}

#[test]
fn a_function_no_test_runs_is_not_labeled_and_a_test_is_not_a_changed_function() {
    let (result, findings) = labeled(Some(vec![ran("test_formats", &[6, 8])]));
    assert_eq!(findings.len(), 1, "format only; parse is run by no test");
    assert_eq!(findings[0].symbol.as_ref().unwrap().display, "format");
    assert_eq!(
        result.score,
        Some(score(0.0, "independently tested share", 1))
    );
}

#[test]
fn without_a_per_test_map_nothing_is_labeled_and_a_note_says_why() {
    let (result, findings) = labeled(None);
    assert_eq!(findings, []);
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.score, None);
    assert!(result.metrics.is_empty());
    assert_eq!(
        result.notes,
        [
            "no per-test coverage map: which tests exercise the changed functions is unknown, so \
          none is labeled; one is made at --depth standard where the language has a PerTestMap \
          producer"
        ]
    );
}

#[test]
fn settings_are_refused() {
    support::assert_refuses_settings(ID);
}

#[test]
fn the_manifest_reads_test_frames_on_both_sides_and_wants_the_per_test_map() {
    let check = support::check(ID);
    assert_eq!(check.base_needs(), [FactKind::TestFrames]);
    assert_eq!(
        Shape::of(check.manifest()),
        Shape {
            needs: vec![FactKind::Symbols, FactKind::NormTree, FactKind::TestFrames],
            wants: vec![FactKind::PerTestMap, FactKind::TestConventions],
            tier: 2,
            cost: CostClass::Quick,
            kinds: vec![("evidence.self-authored-only", ActionKind::Review)],
            metrics: vec![
                ("independent", MetricPer::Scope),
                ("self_authored_only", MetricPer::Scope)
            ],
            evidence: vec!["object"],
        }
    );
}
