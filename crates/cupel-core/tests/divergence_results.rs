// ABOUTME: Tests for the `DivergenceResults` payload (ADR-093): its fact kind and schema, its JSON shape, and
// ABOUTME: how a run's oracle and case budget reach producers.
use std::path::PathBuf;

use cupel_core::config::{OracleConfig, OracleKind};
use cupel_core::syntax::SymbolKey;
use cupel_core::{
    Answer, Cancellation, ComparedOracle, Ctx, Divergence, DivergenceResults, EntryComparison,
    FactKind, Focus, ProduceCtx, ProducerId, RunOracle, Writer, schema,
};
use serde_json::json;

#[test]
fn divergence_results_is_a_fact_kind_with_a_versioned_schema() {
    assert_eq!(FactKind::DivergenceResults.as_str(), "divergence-results");
    assert!(FactKind::ALL.contains(&FactKind::DivergenceResults));
    assert!(!FactKind::SYNTACTIC.contains(&FactKind::DivergenceResults));
    assert_eq!(schema::fact_version(FactKind::DivergenceResults), Some(1));
}

fn diverged() -> DivergenceResults {
    DivergenceResults {
        engine: ProducerId::from("engine.differential"),
        oracle: Some(ComparedOracle {
            name: "quarter".to_owned(),
            kind: OracleKind::Command,
            commit: None,
        }),
        seed: 3,
        cases: 100,
        entries: vec![EntryComparison {
            entry: "quarter.py:quarter".to_owned(),
            symbol: Some(SymbolKey {
                file: "quarter.py".to_owned(),
                line: 1,
                col: 1,
            }),
            compared: 5,
            skipped: None,
            divergence: Some(Divergence {
                input: vec![json!(-1)],
                found: vec![json!(-1)],
                case: 4,
                shrinks: 0,
                implementation: Answer::Returned("-1".to_owned()),
                oracle: Answer::Returned("0".to_owned()),
            }),
        }],
    }
}

#[test]
fn answers_serialize_as_one_key_naming_how_the_call_ended() {
    let shapes = [
        Answer::Returned("-1".to_owned()),
        Answer::Raised("ValueError: no".to_owned()),
        Answer::Crashed("exit 3".to_owned()),
    ]
    .map(|answer| serde_json::to_value(answer).unwrap());
    assert_eq!(
        shapes,
        [
            json!({"returned": "-1"}),
            json!({"raised": "ValueError: no"}),
            json!({"crashed": "exit 3"}),
        ]
    );
}

#[test]
fn divergence_results_round_trip_through_json() {
    let results = diverged();
    let value = serde_json::to_value(&results).unwrap();
    assert_eq!(
        value["oracle"],
        json!({"name": "quarter", "kind": "command", "commit": null})
    );
    assert_eq!(value["entries"][0]["divergence"]["input"], json!([-1]));
    let back: DivergenceResults = serde_json::from_value(value).unwrap();
    assert_eq!(back, results);
}

#[test]
fn producers_outside_a_run_have_no_oracle_and_no_case_budget() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ProduceCtx::new(
        Ctx::new(PathBuf::from(dir.path())),
        Writer::new(dir.path(), false).unwrap().confined(),
        Cancellation::new(),
    );
    let release = RunOracle {
        name: "release".to_owned(),
        definition: OracleConfig {
            kind: OracleKind::Ref,
            reference: Some("v1".to_owned()),
            cmd: None,
            spec: None,
            lang: None,
            inputs: Vec::new(),
        },
    };
    assert_eq!(ctx.oracle, None);
    assert_ne!(ctx.oracle, Some(release));
    assert_eq!(ctx.focus.cases, None);
    assert_eq!(Focus::default().cases, None);
}
