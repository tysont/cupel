// ABOUTME: Tests for the `Traces` payload (ADR-116): its shape as JSON and the spellings of its statuses;
// ABOUTME: schemas.rs pins its kind and version.
mod support;

use cupel_core::{Location, ProducerId, Trace, TraceDivergence, TraceSpec, TraceStatus, Traces};
use serde_json::json;

/// The traces of `tool.tlc` against the `counter` oracle: only `traces`.
fn validated(traces: Vec<Trace>) -> Traces {
    Traces {
        tool: ProducerId::from("tool.tlc"),
        spec: Some(counter()),
        problem: None,
        traces,
    }
}

/// The `counter` spec oracle.
fn counter() -> TraceSpec {
    TraceSpec {
        oracle: "counter".to_owned(),
        file: "spec/Counter.tla".to_owned(),
        lang: "tla+".to_owned(),
    }
}

/// `traces` as JSON, having checked it reads back as itself.
fn round_trip(traces: &Traces) -> serde_json::Value {
    let value = serde_json::to_value(traces).unwrap();
    assert_eq!(
        serde_json::from_value::<Traces>(value.clone()).unwrap(),
        *traces
    );
    value
}

#[test]
fn a_diverged_trace_serializes_every_field() {
    let traces = validated(vec![Trace {
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
    }]);
    assert_eq!(
        round_trip(&traces),
        json!({
            "tool": "tool.tlc",
            "spec": {"oracle": "counter", "file": "spec/Counter.tla", "lang": "tla+"},
            "problem": null,
            "traces": [{
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
            }],
        })
    );
}

#[test]
fn a_failed_trace_has_its_detail_and_no_divergence() {
    let traces = validated(vec![Trace {
        name: "torn.jsonl".to_owned(),
        steps: 0,
        status: TraceStatus::Failed,
        detail: Some("line 2 is not a JSON object".to_owned()),
        divergence: None,
    }]);
    assert_eq!(
        round_trip(&traces)["traces"],
        json!([{
            "name": "torn.jsonl",
            "steps": 0,
            "status": "failed",
            "detail": "line 2 is not a JSON object",
            "divergence": null,
        }])
    );
    assert_eq!(support::spelled(&[TraceStatus::Conforms]), ["conforms"]);
}
