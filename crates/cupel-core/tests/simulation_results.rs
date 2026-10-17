// ABOUTME: Tests for the `SimulationResults` payload (ADR-120): its shape as JSON and the spellings of a harness's
// ABOUTME: statuses; schemas.rs pins its kind and version.
use cupel_core::{
    Location, ProducerId, RootRun, RunStatus, SimulationFailure, SimulationHarness,
    SimulationResults, SimulationStatus,
};
use serde_json::json;

/// A campaign of one root whose harness failed at seed 4.
fn campaign() -> SimulationResults {
    let at = |line| Location {
        file: "tests/sim.rs".to_owned(),
        line,
        col: Some(5),
        end_line: line,
        end_col: None,
    };
    SimulationResults {
        tool: ProducerId::from("tool.madsim"),
        seed: 3,
        seeds: 1000,
        runs: vec![RootRun {
            dir: ".".to_owned(),
            status: RunStatus::Ran,
            detail: None,
        }],
        harnesses: vec![SimulationHarness {
            root: ".".to_owned(),
            name: "concurrent_deposits_are_all_kept".to_owned(),
            location: Some(at(8)),
            status: SimulationStatus::Failed,
            failure: Some(SimulationFailure {
                seed: Some(4),
                message: "assertion `left == right` failed: a deposit was lost".to_owned(),
                location: Some(at(23)),
                trace: vec!["deposit 10 reads 0".to_owned()],
                reproduce: "MADSIM_TEST_SEED=4 cargo test".to_owned(),
            }),
        }],
    }
}

#[test]
fn simulation_results_serializes_every_field() {
    let results = campaign();
    let value = serde_json::to_value(&results).unwrap();
    let location = |line| json!({"file": "tests/sim.rs", "line": line, "col": 5, "end_line": line, "end_col": null});
    assert_eq!(
        value,
        json!({
            "tool": "tool.madsim",
            "seed": 3,
            "seeds": 1000,
            "runs": [{"dir": ".", "status": "ran", "detail": null}],
            "harnesses": [{
                "root": ".",
                "name": "concurrent_deposits_are_all_kept",
                "location": location(8),
                "status": "failed",
                "failure": {
                    "seed": 4,
                    "message": "assertion `left == right` failed: a deposit was lost",
                    "location": location(23),
                    "trace": ["deposit 10 reads 0"],
                    "reproduce": "MADSIM_TEST_SEED=4 cargo test",
                },
            }],
        })
    );
    assert_eq!(
        serde_json::from_value::<SimulationResults>(value).unwrap(),
        results
    );
}

#[test]
fn statuses_are_spelled_in_kebab_case() {
    let statuses = [SimulationStatus::Passed, SimulationStatus::Failed]
        .map(|status| json!(status).as_str().unwrap().to_owned());
    assert_eq!(statuses, ["passed", "failed"]);
}
