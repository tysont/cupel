// ABOUTME: Tests for the `FuzzResults` payload (ADR-107): its shape as JSON and the spellings of a target's
// ABOUTME: statuses; schemas.rs pins its kind and version.
use cupel_core::{FuzzCrash, FuzzResults, FuzzStatus, FuzzTarget, Location, ProducerId};
use serde_json::json;

/// A run of one target that crashed on the empty input.
fn crashed() -> FuzzResults {
    let at = |line| Location {
        file: "src/lib.rs".to_owned(),
        line,
        col: Some(5),
        end_line: line,
        end_col: None,
    };
    FuzzResults {
        tool: ProducerId::from("tool.cargo-fuzz"),
        seed: 3,
        seeded: true,
        cases: 10_000,
        roots: vec![".".to_owned()],
        targets: vec![FuzzTarget {
            root: ".".to_owned(),
            name: "first".to_owned(),
            location: Some(at(1)),
            status: FuzzStatus::Crashed,
            detail: None,
            executions: Some(1),
            coverage: None,
            crash: Some(FuzzCrash {
                message: "index out of bounds: the len is 0 but the index is 0".to_owned(),
                location: Some(at(5)),
                input: String::new(),
                reproducer: Some(".cupel/cache/fuzz/rust/first/crash-da39".to_owned()),
                reproduce: Some("cargo +nightly fuzz run first crash-da39".to_owned()),
            }),
        }],
    }
}

#[test]
fn fuzz_results_serializes_every_field() {
    let results = crashed();
    let value = serde_json::to_value(&results).unwrap();
    let location = |line| json!({"file": "src/lib.rs", "line": line, "col": 5, "end_line": line, "end_col": null});
    assert_eq!(
        value,
        json!({
            "tool": "tool.cargo-fuzz",
            "seed": 3,
            "seeded": true,
            "cases": 10000,
            "roots": ["."],
            "targets": [{
                "root": ".",
                "name": "first",
                "location": location(1),
                "status": "crashed",
                "detail": null,
                "executions": 1,
                "coverage": null,
                "crash": {
                    "message": "index out of bounds: the len is 0 but the index is 0",
                    "location": location(5),
                    "input": "",
                    "reproducer": ".cupel/cache/fuzz/rust/first/crash-da39",
                    "reproduce": "cargo +nightly fuzz run first crash-da39",
                },
            }],
        })
    );
    assert_eq!(
        serde_json::from_value::<FuzzResults>(value).unwrap(),
        results
    );
}

#[test]
fn statuses_are_spelled_in_kebab_case() {
    let statuses = [
        FuzzStatus::Passed,
        FuzzStatus::Crashed,
        FuzzStatus::TimedOut,
        FuzzStatus::Failed,
    ]
    .map(|status| json!(status).as_str().unwrap().to_owned());
    assert_eq!(statuses, ["passed", "crashed", "timed-out", "failed"]);
}
