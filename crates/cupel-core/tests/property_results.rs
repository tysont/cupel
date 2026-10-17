// ABOUTME: Tests for the `PropertyResults` payload (ADR-095): its fact kind and versioned schema, its shape as
// ABOUTME: JSON, the outcome spellings, and the case budget a run's depth or `--cases` gives the property engine.
use cupel_core::syntax::Span;
use cupel_core::{
    CostClass, FactKind, Focus, ProducerId, PropertyOutcome, PropertyResult, PropertyResults,
    schema,
};
use serde_json::json;

fn falsified() -> PropertyResult {
    PropertyResult {
        file: "tests/props.rs".to_owned(),
        name: "digits_counts_decimal_places".to_owned(),
        span: Span {
            line: 5,
            col: 49,
            end_line: 7,
            end_col: 6,
        },
        framework: "proptest".to_owned(),
        test: Some("props::digits_counts_decimal_places".to_owned()),
        outcome: PropertyOutcome::Falsified,
        counterexample: Some("n = 100000".to_owned()),
        seeded: true,
    }
}

#[test]
fn property_results_is_a_fact_kind_with_a_versioned_schema() {
    assert_eq!(FactKind::PropertyResults.as_str(), "property-results");
    assert!(!FactKind::SYNTACTIC.contains(&FactKind::PropertyResults));
    assert_eq!(schema::fact_version(FactKind::PropertyResults), Some(1));
}

#[test]
fn property_results_serialize_every_field() {
    let results = PropertyResults {
        runner: ProducerId::from("tool.cargo-test"),
        seed: 3,
        cases: 1000,
        frameworks: vec!["proptest".to_owned(), "quickcheck".to_owned()],
        properties: vec![falsified()],
    };
    let value = serde_json::to_value(&results).unwrap();
    assert_eq!(
        value,
        json!({
            "runner": "tool.cargo-test",
            "seed": 3,
            "cases": 1000,
            "frameworks": ["proptest", "quickcheck"],
            "properties": [{
                "file": "tests/props.rs",
                "name": "digits_counts_decimal_places",
                "span": {"line": 5, "col": 49, "end_line": 7, "end_col": 6},
                "framework": "proptest",
                "test": "props::digits_counts_decimal_places",
                "outcome": "falsified",
                "counterexample": "n = 100000",
                "seeded": true,
            }],
        })
    );
    let back: PropertyResults = serde_json::from_value(value).unwrap();
    assert_eq!(back, results);
}

#[test]
fn outcomes_are_kebab_case_and_failures_are_falsified_or_failed() {
    let spelled: Vec<(String, bool)> = [
        PropertyOutcome::Passed,
        PropertyOutcome::Falsified,
        PropertyOutcome::Failed,
        PropertyOutcome::Skipped,
        PropertyOutcome::NotRun,
    ]
    .into_iter()
    .map(|outcome| {
        (
            serde_json::to_value(outcome)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned(),
            outcome.is_failure(),
        )
    })
    .collect();
    assert_eq!(
        spelled,
        [
            ("passed".to_owned(), false),
            ("falsified".to_owned(), true),
            ("failed".to_owned(), true),
            ("skipped".to_owned(), false),
            ("not-run".to_owned(), false),
        ]
    );
}

#[test]
fn the_case_budget_is_cases_else_one_per_depth() {
    let budget = |depth, cases| {
        Focus {
            depth,
            cases,
            ..Focus::default()
        }
        .case_budget()
    };
    assert_eq!(budget(CostClass::Quick, None), 100);
    assert_eq!(budget(CostClass::Standard, None), 1_000);
    assert_eq!(budget(CostClass::Deep, None), 10_000);
    assert_eq!(budget(CostClass::Exhaustive, None), 100_000);
    assert_eq!(budget(CostClass::Deep, Some(7)), 7);
    assert_eq!(Focus::default().cases, None);
}
