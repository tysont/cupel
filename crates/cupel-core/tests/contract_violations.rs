// ABOUTME: Tests for the `ContractViolations` payload (ADR-113): its shape as JSON and the spellings of its
// ABOUTME: contract kinds; schemas.rs pins its kind and version.
use cupel_core::{
    ContractKind, ContractPlace, ContractViolation, ContractViolations, ProducerId, RootRun,
    RunStatus,
};
use serde_json::json;

#[test]
fn contract_violations_serializes_every_field() {
    let violations = ContractViolations {
        tool: ProducerId::from("tool.rust-contracts"),
        runner: ProducerId::from("tool.cargo-test"),
        declared: true,
        runs: vec![RootRun {
            dir: ".".to_owned(),
            status: RunStatus::Ran,
            detail: None,
        }],
        tests: 4,
        violations: vec![ContractViolation {
            kind: ContractKind::Precondition,
            function: Some("scale".to_owned()),
            message: "x must not be negative: x >= 0".to_owned(),
            place: Some(ContractPlace {
                file: "src/lib.rs".to_owned(),
                line: 3,
            }),
            test: "units tests::pre".to_owned(),
        }],
    };
    let value = serde_json::to_value(&violations).unwrap();
    assert_eq!(
        value,
        json!({
            "tool": "tool.rust-contracts",
            "runner": "tool.cargo-test",
            "declared": true,
            "runs": [{"dir": ".", "status": "ran", "detail": null}],
            "tests": 4,
            "violations": [{
                "kind": "precondition",
                "function": "scale",
                "message": "x must not be negative: x >= 0",
                "place": {"file": "src/lib.rs", "line": 3},
                "test": "units tests::pre",
            }],
        })
    );
    assert_eq!(
        serde_json::from_value::<ContractViolations>(value).unwrap(),
        violations
    );
}

#[test]
fn contract_kinds_are_spelled_in_kebab_case() {
    assert_eq!(
        [
            ContractKind::Precondition,
            ContractKind::Postcondition,
            ContractKind::Invariant
        ]
        .map(|kind| json!(kind).as_str().unwrap().to_owned()),
        ["precondition", "postcondition", "invariant"]
    );
}
