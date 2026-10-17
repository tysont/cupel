// ABOUTME: Tests for the `ProofResults` payload (ADR-118): its shape as JSON and the spellings of its obligation
// ABOUTME: statuses; schemas.rs pins its kind and version.
use cupel_core::{
    Location, Obligation, ObligationStatus, ProducerId, ProofResults, RelatedPlace, RootRun,
    RunStatus,
};
use serde_json::json;

fn at(line: u32, col: u32) -> Location {
    Location {
        file: "proofs/tally.dfy".to_owned(),
        line,
        col: Some(col),
        end_line: line,
        end_col: Some(col + 1),
    }
}

#[test]
fn proof_results_serializes_every_field() {
    let results = ProofResults {
        tool: ProducerId::from("tool.dafny"),
        runs: vec![RootRun {
            dir: ".".to_owned(),
            status: RunStatus::Ran,
            detail: None,
        }],
        files: vec!["proofs/tally.dfy".to_owned()],
        verified: 3,
        obligations: vec![Obligation {
            status: ObligationStatus::Failed,
            message: "a postcondition could not be proved on this return path".to_owned(),
            location: at(23, 1),
            related: vec![RelatedPlace {
                message: "this is the postcondition that could not be proved".to_owned(),
                location: at(22, 23),
            }],
        }],
    };
    let value = serde_json::to_value(&results).unwrap();
    let location = |line: u32, col: u32| {
        json!({"file": "proofs/tally.dfy", "line": line, "col": col, "end_line": line,
               "end_col": col + 1})
    };
    assert_eq!(
        value,
        json!({
            "tool": "tool.dafny",
            "runs": [{"dir": ".", "status": "ran", "detail": null}],
            "files": ["proofs/tally.dfy"],
            "verified": 3,
            "obligations": [{
                "status": "failed",
                "message": "a postcondition could not be proved on this return path",
                "location": location(23, 1),
                "related": [{
                    "message": "this is the postcondition that could not be proved",
                    "location": location(22, 23),
                }],
            }],
        })
    );
    assert_eq!(
        serde_json::from_value::<ProofResults>(value).unwrap(),
        results
    );
}

#[test]
fn obligation_statuses_are_spelled_in_kebab_case() {
    let spelled = |value: serde_json::Value| value.as_str().unwrap().to_owned();
    assert_eq!(
        [ObligationStatus::Failed, ObligationStatus::Open].map(|status| spelled(json!(status))),
        ["failed", "open"]
    );
}
