// ABOUTME: Tests for the `ModelCheckResults` payload (ADR-117): its shape as JSON and the spellings of its harness
// ABOUTME: statuses; schemas.rs pins its kind and version.
use cupel_core::{
    Counterexample, HarnessStatus, Location, ModelCheckResults, ModelHarness, ProducerId,
    RefutedProperty, RootRun, RunStatus,
};
use serde_json::json;

fn at(file: &str, line: u32) -> Location {
    Location {
        file: file.to_owned(),
        line,
        col: None,
        end_line: line,
        end_col: None,
    }
}

#[test]
fn model_check_results_serializes_every_field() {
    let results = ModelCheckResults {
        tool: ProducerId::from("tool.kani"),
        runs: vec![RootRun {
            dir: ".".to_owned(),
            status: RunStatus::Ran,
            detail: None,
        }],
        harnesses: vec![ModelHarness {
            root: ".".to_owned(),
            name: "proofs::average_never_overflows".to_owned(),
            location: Some(at("src/lib.rs", 18)),
            status: HarnessStatus::Failed,
            refuted: vec![RefutedProperty {
                description: "attempt to add with overflow".to_owned(),
                function: Some("average".to_owned()),
                location: Some(at("src/lib.rs", 2)),
            }],
            counterexample: Some(Counterexample {
                values: vec!["128".to_owned(), "128".to_owned()],
                replay: Some("#[test]\nfn replay() {}\n".to_owned()),
            }),
        }],
    };
    let value = serde_json::to_value(&results).unwrap();
    let location = |line: u32| json!({"file": "src/lib.rs", "line": line, "col": null, "end_line": line, "end_col": null});
    assert_eq!(
        value,
        json!({
            "tool": "tool.kani",
            "runs": [{"dir": ".", "status": "ran", "detail": null}],
            "harnesses": [{
                "root": ".",
                "name": "proofs::average_never_overflows",
                "location": location(18),
                "status": "failed",
                "refuted": [{
                    "description": "attempt to add with overflow",
                    "function": "average",
                    "location": location(2),
                }],
                "counterexample": {
                    "values": ["128", "128"],
                    "replay": "#[test]\nfn replay() {}\n",
                },
            }],
        })
    );
    assert_eq!(
        serde_json::from_value::<ModelCheckResults>(value).unwrap(),
        results
    );
}

#[test]
fn harness_statuses_are_spelled_in_kebab_case() {
    let spelled = |value: serde_json::Value| value.as_str().unwrap().to_owned();
    assert_eq!(
        [
            HarnessStatus::Verified,
            HarnessStatus::Failed,
            HarnessStatus::Undetermined
        ]
        .map(|status| spelled(json!(status))),
        ["verified", "failed", "undetermined"]
    );
}
