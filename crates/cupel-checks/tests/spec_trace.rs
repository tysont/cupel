// ABOUTME: spec.trace over synthetic Traces facts: a diverged trace is a failing finding at the logged step, traces
// ABOUTME: that could not be validated and a recording problem are notes, the score, the oracle, the manifest (ADR-116).
mod support;

use cupel_core::{
    ActionKind, CheckId, CheckStatus, CostClass, FactKind, FindingId, Location, MetricPer,
    ProducerId, Severity, Trace, TraceDivergence, TraceSpec, TraceStatus, Traces,
};
use serde_json::json;
use support::{Shape, score};

const ID: &str = "spec.trace";

fn trace(name: &str, status: TraceStatus) -> Trace {
    Trace {
        name: name.to_owned(),
        steps: 3,
        status,
        detail: None,
        divergence: None,
    }
}

fn diverged(name: &str, step: u64, location: Option<Location>) -> Trace {
    Trace {
        divergence: Some(TraceDivergence {
            step,
            action: (step > 1).then(|| "Inc".to_owned()),
            state: json!({"count": 3}),
            previous: (step > 1).then(|| json!({"count": 1})),
            location,
        }),
        ..trace(name, TraceStatus::Diverged)
    }
}

fn at(line: u32) -> Location {
    support::line_at("src/lib.rs", line)
}

fn validated(traces: Vec<Trace>) -> Traces {
    Traces {
        tool: ProducerId::from("tool.tlc"),
        spec: Some(TraceSpec {
            oracle: "counter".to_owned(),
            file: "spec/Counter.tla".to_owned(),
            lang: "tla+".to_owned(),
        }),
        problem: None,
        traces,
    }
}

#[test]
fn a_diverged_trace_is_a_failing_finding_where_the_step_was_logged() {
    let (result, findings) = support::run_over(
        ID,
        validated(vec![
            diverged("bumps.jsonl", 3, Some(at(33))),
            trace("counts.jsonl", TraceStatus::Conforms),
        ]),
    );
    assert_eq!(result.status, CheckStatus::Fail);
    let [finding] = findings.as_slice() else {
        panic!("{findings:?}")
    };
    assert_eq!(finding.kind, "trace.diverged");
    assert_eq!(
        (finding.severity, finding.action.kind, finding.confidence),
        (Severity::Fail, ActionKind::FixBug, 1.0)
    );
    assert_eq!(finding.location, Some(at(33)));
    assert_eq!(
        finding.title,
        "trace `bumps.jsonl` leaves spec/Counter.tla at step 3: `Inc` to {\"count\":3} is not a \
         step the spec allows from {\"count\":1}"
    );
    assert_eq!(
        finding.evidence.0,
        *json!({
            "tool": "tool.tlc",
            "oracle": "counter",
            "spec": "spec/Counter.tla",
            "trace": "bumps.jsonl",
            "steps": 3,
            "step": 3,
            "action": "Inc",
            "state": {"count": 3},
            "previous": {"count": 1},
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(
        finding.id,
        FindingId::for_subject(
            &CheckId::from(ID),
            "trace.diverged",
            "bumps.jsonl|Inc|{\"count\":3}"
        )
    );
    assert_eq!(
        finding.repro,
        "cupel check --only spec.trace --oracle counter --paths src/lib.rs"
    );
}

#[test]
fn a_trace_that_starts_outside_the_spec_names_its_first_state() {
    let (_, findings) = support::run_over(ID, validated(vec![diverged("restarts.jsonl", 1, None)]));
    assert_eq!(
        findings[0].title,
        "trace `restarts.jsonl` starts at {\"count\":3}, which is not an initial state of \
         spec/Counter.tla"
    );
    assert_eq!(findings[0].location, None);
    assert_eq!(
        findings[0].repro,
        "cupel check --only spec.trace --oracle counter"
    );
}

#[test]
fn the_score_counts_diverged_traces_and_the_rest_are_notes() {
    let mut torn = trace("torn.jsonl", TraceStatus::Failed);
    torn.detail = Some("line 1: not JSON".to_owned());
    let mut traces = validated(vec![
        diverged("bumps.jsonl", 3, None),
        trace("counts.jsonl", TraceStatus::Conforms),
        torn,
    ]);
    traces.problem = Some("the test run that recorded traces did not pass".to_owned());
    let (result, _) = support::run_over(ID, traces);
    assert_eq!(result.score, Some(score(1.0, "diverged", 2)));
    assert_eq!(
        result.metrics,
        [
            ("diverged".to_owned(), 1.0),
            ("traces".to_owned(), 2.0),
            ("unvalidated".to_owned(), 1.0),
        ]
        .into()
    );
    assert_eq!(
        result.notes,
        [
            "tool.tlc: the test run that recorded traces did not pass",
            "trace `torn.jsonl` could not be validated: line 1: not JSON",
        ]
    );
}

#[test]
fn traces_that_all_conform_pass_and_no_validated_trace_scores_nothing() {
    let (result, findings) = support::run_over(
        ID,
        validated(vec![trace("counts.jsonl", TraceStatus::Conforms)]),
    );
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.score, Some(score(0.0, "diverged", 1)));

    let mut unconfigured = validated(Vec::new());
    unconfigured.spec = None;
    unconfigured.problem = Some("the run has no oracle".to_owned());
    let (result, _) = support::run_over(ID, unconfigured);
    assert_eq!(result.score, None);
    assert!(result.metrics.is_empty(), "{:?}", result.metrics);
    assert_eq!(result.notes, ["tool.tlc: the run has no oracle"]);
}

#[test]
fn the_check_compares_against_the_runs_oracle() {
    let check = support::check(ID);
    let manifest = check.manifest();
    assert_eq!(
        (u8::from(manifest.tier), manifest.cost, check.uses_oracle()),
        (5, CostClass::Exhaustive, true)
    );
}

#[test]
fn settings_are_refused() {
    support::assert_refuses_settings(ID);
}

#[test]
fn every_metric_adds_up_across_languages() {
    support::assert_counts_add_up(ID, &["traces", "diverged", "unvalidated"], "diverged");
}

#[test]
fn the_manifest_reads_the_traces() {
    assert_eq!(
        Shape::of(support::check(ID).manifest()),
        Shape {
            needs: vec![FactKind::Traces],
            wants: vec![],
            tier: 5,
            cost: CostClass::Exhaustive,
            kinds: vec![("trace.diverged", ActionKind::FixBug)],
            metrics: ["traces", "diverged", "unvalidated"]
                .map(|metric| (metric, MetricPer::Scope))
                .to_vec(),
            evidence: vec!["object"],
        }
    );
}
