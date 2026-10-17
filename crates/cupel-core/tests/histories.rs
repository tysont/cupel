// ABOUTME: Tests for the `Histories` payload (ADR-119): its shape as JSON and the spellings of a history's statuses
// ABOUTME: and an operation's outcomes; schemas.rs pins its kind and version.
use cupel_core::{
    Histories, History, HistoryOp, HistoryStatus, HistoryViolation, OpOutcome, ProducerId, RootRun,
    RunStatus,
};
use serde_json::json;

/// A run whose one history holds a stale read.
fn checked() -> Histories {
    let op = |process: u64, f: &str, value: serde_json::Value, line: u32| HistoryOp {
        process: json!(process),
        f: f.to_owned(),
        key: None,
        value,
        outcome: OpOutcome::Ok,
        line,
    };
    Histories {
        tool: ProducerId::from("tool.porcupine"),
        command: "cargo test --test history".to_owned(),
        seed: 3,
        run: RootRun {
            dir: ".".to_owned(),
            status: RunStatus::Ran,
            detail: None,
        },
        histories: vec![History {
            name: "register.jsonl".to_owned(),
            model: Some("register".to_owned()),
            operations: 3,
            status: HistoryStatus::Violated,
            detail: None,
            violations: vec![HistoryViolation {
                key: None,
                anomaly: "non-linearizable".to_owned(),
                states: vec![json!(2)],
                linearized: vec![op(0, "write", json!(1), 2), op(0, "write", json!(2), 4)],
                stuck: vec![op(1, "read", json!(1), 6)],
            }],
        }],
    }
}

#[test]
fn histories_serializes_every_field() {
    let histories = checked();
    let value = serde_json::to_value(&histories).unwrap();
    let op = |process: u64, f: &str, value: u64, line: u32| json!({"process": process, "f": f, "key": null, "value": value, "outcome": "ok", "line": line});
    assert_eq!(
        value,
        json!({
            "tool": "tool.porcupine",
            "command": "cargo test --test history",
            "seed": 3,
            "run": {"dir": ".", "status": "ran", "detail": null},
            "histories": [{
                "name": "register.jsonl",
                "model": "register",
                "operations": 3,
                "status": "violated",
                "detail": null,
                "violations": [{
                    "key": null,
                    "anomaly": "non-linearizable",
                    "states": [2],
                    "linearized": [op(0, "write", 1, 2), op(0, "write", 2, 4)],
                    "stuck": [op(1, "read", 1, 6)],
                }],
            }],
        })
    );
    assert_eq!(
        serde_json::from_value::<Histories>(value).unwrap(),
        histories
    );
}

#[test]
fn statuses_and_outcomes_are_spelled_in_kebab_case() {
    let statuses = [
        HistoryStatus::Linearizable,
        HistoryStatus::Violated,
        HistoryStatus::Unknown,
        HistoryStatus::Invalid,
    ]
    .map(|status| json!(status).as_str().unwrap().to_owned());
    assert_eq!(statuses, ["linearizable", "violated", "unknown", "invalid"]);
    let outcomes =
        [OpOutcome::Ok, OpOutcome::Info].map(|outcome| json!(outcome).as_str().unwrap().to_owned());
    assert_eq!(outcomes, ["ok", "info"]);
}
