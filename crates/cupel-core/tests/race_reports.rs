// ABOUTME: Tests for the `RaceReports` payload (ADR-105): its fact kind and versioned schema, its shape as JSON,
// ABOUTME: the spellings of its kinds and statuses, and the `race` key that pins its producer.
mod support;

use cupel_core::{
    Ctx, FactKind, LangId, ProducerId, RaceFrame, RaceKind, RaceReport, RaceReports, RaceStack,
    RootRun, RunStatus,
};
use serde_json::json;

#[test]
fn race_reports_is_an_execution_fact_with_a_versioned_schema() {
    assert_eq!(
        support::fact_entry(FactKind::RaceReports),
        (
            "race-reports",
            false,
            Some(1),
            Some(json!("https://cupel.dev/schema/v1/fact/race-reports.json")),
        )
    );
}

#[test]
fn race_reports_serializes_every_field() {
    let reports = RaceReports {
        tool: ProducerId::from("tool.go-race"),
        runs: vec![
            RootRun {
                dir: ".".to_owned(),
                status: RunStatus::Ran,
                detail: None,
            },
            RootRun {
                dir: "tools".to_owned(),
                status: RunStatus::Failed,
                detail: Some("go: -race requires cgo".to_owned()),
            },
        ],
        reports: vec![RaceReport {
            kind: RaceKind::DataRace,
            test: Some("TestCount".to_owned()),
            message: "data race".to_owned(),
            stacks: vec![RaceStack {
                access: Some("write".to_owned()),
                frames: vec![RaceFrame {
                    function: Some("example.com/units.Count.func1".to_owned()),
                    file: "count.go".to_owned(),
                    line: 15,
                }],
            }],
        }],
    };
    let value = serde_json::to_value(&reports).unwrap();
    assert_eq!(
        value,
        json!({
            "tool": "tool.go-race",
            "runs": [
                {"dir": ".", "status": "ran", "detail": null},
                {"dir": "tools", "status": "failed", "detail": "go: -race requires cgo"},
            ],
            "reports": [{
                "kind": "data-race",
                "test": "TestCount",
                "message": "data race",
                "stacks": [{"access": "write", "frames": [
                    {"function": "example.com/units.Count.func1", "file": "count.go", "line": 15},
                ]}],
            }],
        })
    );
    assert_eq!(
        serde_json::from_value::<RaceReports>(value).unwrap(),
        reports
    );
}

#[test]
fn kinds_and_statuses_are_spelled_in_kebab_case() {
    let spelled = |value: serde_json::Value| value.as_str().unwrap().to_owned();
    assert_eq!(
        [RaceKind::DataRace, RaceKind::UndefinedBehavior].map(|kind| spelled(json!(kind))),
        ["data-race", "undefined-behavior"]
    );
    assert_eq!(
        [RunStatus::Ran, RunStatus::Failed].map(|status| spelled(json!(status))),
        ["ran", "failed"]
    );
}

#[test]
fn the_race_key_of_a_language_pins_its_race_detector() {
    let mut ctx = Ctx::new(std::path::PathBuf::from("/repo"));
    let rust = LangId::from("rust");
    ctx.lang.insert(
        rust.clone(),
        serde_json::from_value(json!({"race": "tsan"})).unwrap(),
    );
    assert_eq!(FactKind::RaceReports.pin_key(), Some("race"));
    assert_eq!(ctx.pin(FactKind::RaceReports, &rust), Some("tsan"));
    assert_eq!(ctx.pin(FactKind::RaceReports, &LangId::from("go")), None);
}
