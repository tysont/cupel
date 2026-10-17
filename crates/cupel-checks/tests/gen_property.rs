// ABOUTME: gen.property over hand-built PropertyResults, Symbols, TestConventions, and ApiSurface (ADR-096): falsified
// ABOUTME: and failed properties, missing ones, their evidence, identity, repro, status, metrics, and the score.
mod support;

use cupel_core::syntax::{
    ApiItem, ApiItemKind, ApiSurface, Span, SymbolDef, SymbolKind, Symbols, TestConventions,
    TestPairing, Visibility,
};
use cupel_core::{
    ActionKind, CheckStatus, CostClass, FactKind, Facts, Finding, LangId, ProducerId,
    PropertyOutcome, PropertyResult, PropertyResults, Scope, Severity,
};
use serde_json::{Value, json};
use support::{Recorded, check, recorded, score};

const ID: &str = "gen.property";

fn span(line: u32) -> Span {
    Span {
        line,
        col: 5,
        end_line: line + 2,
        end_col: 6,
    }
}

fn property(name: &str, outcome: PropertyOutcome, counterexample: Option<&str>) -> PropertyResult {
    PropertyResult {
        file: "tests/test_units.py".to_owned(),
        name: name.to_owned(),
        span: span(10),
        framework: "hypothesis".to_owned(),
        test: Some(format!("tests/test_units.py::{name}")),
        outcome,
        counterexample: counterexample.map(str::to_owned),
        seeded: true,
    }
}

fn results(properties: Vec<PropertyResult>) -> PropertyResults {
    PropertyResults {
        runner: ProducerId::from("tool.pytest"),
        seed: 3,
        cases: 1000,
        frameworks: vec!["hypothesis".to_owned()],
        properties,
    }
}

fn function(file: &str, name: &str, visibility: Visibility, parameters: u32) -> SymbolDef {
    SymbolDef {
        name: name.to_owned(),
        namespace: "units".to_owned(),
        display: format!("units.{name}"),
        kind: SymbolKind::Function,
        visibility,
        lang: LangId::from("python"),
        file: file.to_owned(),
        span: span(1),
        body: None,
        parameters,
    }
}

/// `src/units/digits.py` tested by `tests/test_units.py` (which holds the properties) and
/// `src/units/percent.py` by `tests/test_percent.py` (example tests only).
fn conventions() -> TestConventions {
    TestConventions {
        test_globs: vec!["tests/**/*.py".to_owned()],
        pairs: vec![
            TestPairing {
                source: "src/units/digits.py".to_owned(),
                tests: vec!["tests/test_units.py".to_owned()],
            },
            TestPairing {
                source: "src/units/percent.py".to_owned(),
                tests: vec!["tests/test_percent.py".to_owned()],
            },
        ],
    }
}

fn symbols() -> Symbols {
    Symbols {
        symbols: vec![
            function("src/units/digits.py", "digits", Visibility::Public, 1),
            function(
                "src/units/percent.py",
                "clamp_percent",
                Visibility::Public,
                1,
            ),
            function("src/units/percent.py", "_helper", Visibility::Internal, 1),
            function(
                "src/units/percent.py",
                "default_percent",
                Visibility::Public,
                0,
            ),
            SymbolDef {
                kind: SymbolKind::Method,
                ..function("src/units/percent.py", "scaled", Visibility::Public, 2)
            },
            function(
                "tests/test_percent.py",
                "test_clamps",
                Visibility::Public,
                1,
            ),
        ],
    }
}

const ALL_FILES: [&str; 4] = [
    "src/units/digits.py",
    "src/units/percent.py",
    "tests/test_percent.py",
    "tests/test_units.py",
];

fn run_over(
    properties: PropertyResults,
    api: Option<ApiSurface>,
    files: &[&str],
) -> (cupel_core::CheckResult, Recorded) {
    let check = check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(properties).unwrap();
    facts.insert(symbols()).unwrap();
    facts.insert(conventions()).unwrap();
    if let Some(api) = api {
        facts.insert(api).unwrap();
    }
    let scope = Scope::of_files(files.iter().map(|file| (*file).to_owned()).collect());
    recorded(check, &facts, scope, json!({})).unwrap()
}

fn kinds(findings: &[Finding]) -> Vec<(String, String)> {
    findings
        .iter()
        .map(|finding| {
            let field = |name: &str| finding.evidence.0.get(name).and_then(Value::as_str);
            let subject = field("test")
                .or_else(|| field("function"))
                .unwrap()
                .to_owned();
            (finding.kind.clone(), subject)
        })
        .collect()
}

fn kind(kind: &str, subject: &str) -> (String, String) {
    (kind.to_owned(), subject.to_owned())
}

#[test]
fn the_manifest_needs_property_results_symbols_and_conventions_at_tier_four() {
    let manifest = check(ID).manifest();
    assert_eq!(
        manifest.needs,
        [
            FactKind::PropertyResults,
            FactKind::Symbols,
            FactKind::TestConventions
        ]
    );
    assert_eq!(manifest.wants, [FactKind::ApiSurface]);
    assert_eq!(manifest.cost, CostClass::Standard);
    assert_eq!(u8::from(manifest.tier), 4);
    let kinds: Vec<(&str, ActionKind)> = manifest
        .findings
        .iter()
        .map(|spec| (spec.kind.as_str(), spec.action))
        .collect();
    assert_eq!(
        kinds,
        [
            ("property.falsified", ActionKind::FixBug),
            ("property.missing", ActionKind::AddProperty),
        ]
    );
    let metrics: Vec<&str> = manifest.metrics.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(metrics, ["properties", "falsified", "missing"]);
}

#[test]
fn a_falsified_property_fails_with_its_counterexample_seed_and_budget() {
    let (result, recorded) = run_over(
        results(vec![property(
            "test_digits_counts_decimal_places",
            PropertyOutcome::Falsified,
            Some("n=100000"),
        )]),
        None,
        &ALL_FILES,
    );
    assert_eq!(result.status, CheckStatus::Fail);
    let finding = recorded
        .findings
        .iter()
        .find(|finding| finding.kind == "property.falsified")
        .unwrap();
    assert_eq!(finding.severity, Severity::Fail);
    assert_eq!(finding.confidence.to_string(), "0.95");
    assert_eq!(
        finding.title,
        "`test_digits_counts_decimal_places` is falsified by `n=100000`"
    );
    assert_eq!(
        Value::Object(finding.evidence.0.clone()),
        json!({
            "test": "tests/test_units.py::test_digits_counts_decimal_places",
            "framework": "hypothesis",
            "runner": "tool.pytest",
            "counterexample": "n=100000",
            "seed": 3,
            "cases": 1000,
        })
    );
    let location = finding.location.as_ref().unwrap();
    assert_eq!(
        (location.file.as_str(), location.line, location.end_line),
        ("tests/test_units.py", 10, 12)
    );
    assert_eq!(
        finding.repro,
        "cupel check --only gen.property --paths tests/test_units.py --seed 3 --cases 1000"
    );
    assert_eq!(finding.action.kind, ActionKind::FixBug);
    assert_eq!(
        format!("{:?}", result.metrics),
        r#"{"falsified": 1.0, "missing": 1.0, "properties": 1.0}"#
    );
    assert_eq!(result.score, Some(score(1.0, "falsified", 1)));
}

#[test]
fn a_property_that_failed_without_a_counterexample_is_falsified_with_less_confidence() {
    let mut unseeded = property("test_crashes", PropertyOutcome::Failed, None);
    unseeded.seeded = false;
    let (result, recorded) = run_over(results(vec![unseeded]), None, &ALL_FILES);
    assert_eq!(result.status, CheckStatus::Fail);
    let finding = &recorded.findings[0];
    assert_eq!(finding.kind, "property.falsified");
    assert_eq!(finding.confidence.to_string(), "0.6");
    assert_eq!(finding.evidence.0["counterexample"], Value::Null);
    assert_eq!(finding.evidence.0["seed"], Value::Null);
    assert_eq!(
        finding.title,
        "`test_crashes` failed under the case budget without a counterexample"
    );
}

#[test]
fn properties_that_held_were_skipped_or_did_not_run_report_nothing() {
    let (result, recorded) = run_over(
        results(vec![
            property("test_holds", PropertyOutcome::Passed, None),
            property("test_skipped", PropertyOutcome::Skipped, None),
            property("test_absent", PropertyOutcome::NotRun, None),
        ]),
        None,
        &ALL_FILES,
    );
    assert_eq!(
        kinds(&recorded.findings),
        [kind("property.missing", "units.clamp_percent")]
    );
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(
        format!("{:?}", result.metrics),
        r#"{"falsified": 0.0, "missing": 1.0, "properties": 3.0}"#
    );
    assert_eq!(result.score, Some(score(0.0, "falsified", 3)));
}

#[test]
fn a_public_function_whose_tests_hold_no_property_is_missing_one() {
    let api = ApiSurface {
        items: vec![ApiItem {
            path: "units.clamp_percent".to_owned(),
            kind: ApiItemKind::Function,
            signature: "(_)".to_owned(),
            file: "src/units/percent.py".to_owned(),
            span: span(1),
        }],
    };
    let (result, recorded) = run_over(
        results(vec![property("test_holds", PropertyOutcome::Passed, None)]),
        Some(api),
        &ALL_FILES,
    );
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(
        format!("{:?}", result.metrics),
        r#"{"falsified": 0.0, "missing": 1.0, "properties": 1.0}"#
    );
    let finding = &recorded.findings[0];
    assert_eq!(finding.kind, "property.missing");
    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(finding.confidence.to_string(), "0.5");
    assert_eq!(finding.action.kind, ActionKind::AddProperty);
    assert_eq!(
        Value::Object(finding.evidence.0.clone()),
        json!({
            "function": "units.clamp_percent",
            "name": "clamp_percent",
            "namespace": "units",
            "parameters": 1,
            "signature": "(_)",
            "frameworks": ["hypothesis"],
        })
    );
    assert_eq!(
        finding.location.as_ref().map(|l| (l.file.as_str(), l.line)),
        Some(("src/units/percent.py", 1))
    );
    assert_eq!(finding.title, "`units.clamp_percent` has no property test");
    assert!(finding.action.summary.contains("--scaffold"));
}

#[test]
fn a_function_whose_tests_are_out_of_scope_is_not_judged() {
    let (_, recorded) = run_over(
        results(Vec::new()),
        None,
        &["src/units/digits.py", "src/units/percent.py"],
    );
    assert_eq!(kinds(&recorded.findings), Vec::<(String, String)>::new());
}

#[test]
fn a_source_no_convention_pairs_with_a_test_is_missing_its_properties() {
    let check = check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(results(Vec::new())).unwrap();
    facts
        .insert(Symbols {
            symbols: vec![function("src/units/lone.py", "lone", Visibility::Public, 2)],
        })
        .unwrap();
    facts.insert(conventions()).unwrap();
    let scope = Scope::of_files(vec!["src/units/lone.py".to_owned()]);
    let (_, recorded) = recorded(check, &facts, scope, json!({})).unwrap();
    assert_eq!(
        kinds(&recorded.findings),
        [kind("property.missing", "units.lone")]
    );
    assert_eq!(recorded.findings[0].evidence.0["signature"], Value::Null);
}

#[test]
fn finding_ids_are_stable_and_distinct() {
    let run = || {
        run_over(
            results(vec![property(
                "test_digits",
                PropertyOutcome::Falsified,
                Some("n=1"),
            )]),
            None,
            &ALL_FILES,
        )
        .1
        .findings
        .iter()
        .map(|finding| finding.id.to_string())
        .collect::<Vec<_>>()
    };
    let first = run();
    assert_eq!(first, run());
    assert_eq!(first.len(), 2);
    assert_ne!(first[0], first[1]);
}
