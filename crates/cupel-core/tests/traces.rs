// ABOUTME: Tests for the `Traces` payload (ADR-116): its shape as JSON and the spellings of its statuses;
// ABOUTME: schemas.rs pins its kind and version.
use cupel_core::{Location, ProducerId, Trace, TraceDivergence, TraceSpec, TraceStatus, Traces};
use serde_json::json;

#[test]
fn traces_serializes_every_field() {
    let traces = Traces {
        tool: ProducerId::from("tool.tlc"),
        spec: Some(TraceSpec {
            oracle: "counter".to_owned(),
            file: "spec/Counter.tla".to_owned(),
            lang: "tla+".to_owned(),
        }),
        problem: None,
        traces: vec![
            Trace {
                name: "bumps.jsonl".to_owned(),
                steps: 3,
                status: TraceStatus::Diverged,
                detail: None,
                divergence: Some(TraceDivergence {
                    step: 3,
                    action: Some("Inc".to_owned()),
                    state: json!({"count": 3}),
                    previous: Some(json!({"count": 1})),
                    location: Some(Location {
                        file: "src/lib.rs".to_owned(),
                        line: 12,
                        col: None,
                        end_line: 12,
                        end_col: None,
                    }),
                }),
            },
            Trace {
                name: "torn.jsonl".to_owned(),
                steps: 0,
                status: TraceStatus::Failed,
                detail: Some("line 2 is not a JSON object".to_owned()),
                divergence: None,
            },
        ],
    };
    let value = serde_json::to_value(&traces).unwrap();
    assert_eq!(
        value,
        json!({
            "tool": "tool.tlc",
            "spec": {"oracle": "counter", "file": "spec/Counter.tla", "lang": "tla+"},
            "problem": null,
            "traces": [
                {
                    "name": "bumps.jsonl",
                    "steps": 3,
                    "status": "diverged",
                    "detail": null,
                    "divergence": {
                        "step": 3,
                        "action": "Inc",
                        "state": {"count": 3},
                        "previous": {"count": 1},
                        "location": {"file": "src/lib.rs", "line": 12, "col": null,
                                     "end_line": 12, "end_col": null},
                    },
                },
                {
                    "name": "torn.jsonl",
                    "steps": 0,
                    "status": "failed",
                    "detail": "line 2 is not a JSON object",
                    "divergence": null,
                },
            ],
        })
    );
    assert_eq!(serde_json::from_value::<Traces>(value).unwrap(), traces);
}

#[test]
fn statuses_are_spelled_in_kebab_case() {
    let spelled = |value: serde_json::Value| value.as_str().unwrap().to_owned();
    assert_eq!(
        [
            TraceStatus::Conforms,
            TraceStatus::Diverged,
            TraceStatus::Failed
        ]
        .map(|status| spelled(json!(status))),
        ["conforms", "diverged", "failed"]
    );
}
