// ABOUTME: conc.linearizability over synthetic Histories facts: violations with their excerpt, keys told apart, notes for
// ABOUTME: histories that could not be checked and a failed history command, the score and metrics, the manifest (ADR-119).
mod support;

use cupel_core::{
    ActionKind, CheckId, CheckStatus, CostClass, FactKind, FindingId, Histories, History,
    HistoryOp, HistoryStatus, HistoryViolation, MetricPer, OpOutcome, ProducerId, RootRun,
    RunStatus, Severity,
};
use serde_json::{Value, json};
use support::{Shape, score};

const ID: &str = "conc.linearizability";

fn op(process: u64, f: &str, value: Value, line: u32) -> HistoryOp {
    HistoryOp {
        process: json!(process),
        f: f.to_owned(),
        key: None,
        value,
        outcome: OpOutcome::Ok,
        line,
    }
}

fn history(name: &str, status: HistoryStatus) -> History {
    History {
        name: name.to_owned(),
        model: Some("register".to_owned()),
        operations: 3,
        status,
        detail: None,
        violations: Vec::new(),
    }
}

fn stale_read() -> HistoryViolation {
    HistoryViolation {
        key: None,
        anomaly: "non-linearizable".to_owned(),
        states: vec![json!(2)],
        linearized: vec![op(0, "write", json!(1), 2), op(0, "write", json!(2), 4)],
        stuck: vec![op(1, "read", json!(1), 6)],
    }
}

/// A stale read, a key-value history with one bad key, a linearizable history, one the checker
/// ran out of time on, and an invalid one.
fn checked() -> Histories {
    let mut stale = history("stale.jsonl", HistoryStatus::Violated);
    stale.violations = vec![stale_read()];
    let mut keyed = history("kv.jsonl", HistoryStatus::Violated);
    keyed.model = Some("kv".to_owned());
    let mut read = op(1, "read", Value::Null, 8);
    read.key = Some(json!("y"));
    keyed.violations = vec![HistoryViolation {
        key: Some(json!("y")),
        states: vec![json!(1)],
        linearized: vec![],
        stuck: vec![read, op(2, "write", json!(3), 9)],
        ..stale_read()
    }];
    let mut slow = history("slow.jsonl", HistoryStatus::Unknown);
    slow.detail = Some("the checker ran out of its 30 s".to_owned());
    let mut invalid = history("queue.jsonl", HistoryStatus::Invalid);
    invalid.model = Some("queue".to_owned());
    invalid.operations = 0;
    invalid.detail = Some("tool.porcupine checks register and kv histories, not queue".to_owned());
    Histories {
        tool: ProducerId::from("tool.porcupine"),
        command: "go test -run TestHistory ./...".to_owned(),
        seed: 3,
        run: RootRun {
            dir: ".".to_owned(),
            status: RunStatus::Ran,
            detail: None,
        },
        histories: vec![
            stale,
            keyed,
            history("fine.jsonl", HistoryStatus::Linearizable),
            slow,
            invalid,
        ],
    }
}

#[test]
fn a_violation_is_a_failing_finding_with_its_excerpt_and_how_to_record_it_again() {
    let (_, findings) = support::run_over(ID, checked());
    assert_eq!(findings.len(), 2, "{findings:#?}");
    let stale = &findings[0];
    assert_eq!(stale.kind, "consistency.violation");
    assert_eq!(
        (stale.severity, stale.action.kind, stale.confidence),
        (Severity::Fail, ActionKind::FixBug, 1.0)
    );
    assert_eq!(
        stale.id,
        FindingId::for_subject(&CheckId::from(ID), "consistency.violation", "stale.jsonl")
    );
    assert_eq!(stale.location, None);
    assert_eq!(
        stale.title,
        "history `stale.jsonl` (register) is non-linearizable: `read 1` by process 1 (line 6) \
         cannot follow state `2`"
    );
    let ops = |ops: &[HistoryOp]| serde_json::to_value(ops).unwrap();
    assert_eq!(
        stale.evidence.0,
        *json!({
            "history": "stale.jsonl",
            "model": "register",
            "key": null,
            "anomaly": "non-linearizable",
            "tool": "tool.porcupine",
            "states": [2],
            "linearized": ops(&stale_read().linearized),
            "stuck": ops(&stale_read().stuck),
            "command": "go test -run TestHistory ./...",
            "seed": 3,
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(stale.repro, "cupel check --only conc.linearizability --seed 3");
    assert!(
        stale
            .action
            .summary
            .contains("CUPEL_SEED=3 go test -run TestHistory ./..."),
        "{}",
        stale.action.summary
    );
}

#[test]
fn each_key_of_a_key_value_history_is_its_own_finding() {
    let (_, findings) = support::run_over(ID, checked());
    let keyed = &findings[1];
    assert_eq!(
        keyed.id,
        FindingId::for_subject(
            &CheckId::from(ID),
            "consistency.violation",
            "kv.jsonl|\"y\""
        )
    );
    assert_eq!(
        keyed.title,
        "history `kv.jsonl` (kv) key `\"y\"` is non-linearizable: `read null` by process 1 \
         (line 8) and 1 more operation cannot follow state `1`"
    );
    assert_eq!(keyed.evidence.0["key"], json!("y"));
}

#[test]
fn a_violation_without_states_says_no_order_explains_it() {
    let mut fact = checked();
    fact.histories[0].violations[0].states = Vec::new();
    fact.histories[0].model = None;
    let (_, findings) = support::run_over(ID, fact);
    assert_eq!(
        findings[0].title,
        "history `stale.jsonl` is non-linearizable: `read 1` by process 1 (line 6) cannot be \
         ordered"
    );
}

#[test]
fn the_score_counts_violations_and_unchecked_histories_are_notes() {
    let (result, _) = support::run_over(ID, checked());
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.score, Some(score(2.0, "violations", 5)));
    assert_eq!(
        result.metrics,
        [
            ("histories".to_owned(), 5.0),
            ("invalid".to_owned(), 1.0),
            ("operations".to_owned(), 12.0),
            ("unknown".to_owned(), 1.0),
            ("violations".to_owned(), 2.0),
        ]
        .into()
    );
    assert_eq!(
        result.notes,
        [
            "slow.jsonl could not be decided: the checker ran out of its 30 s",
            "queue.jsonl could not be checked: tool.porcupine checks register and kv histories, \
             not queue",
        ]
    );
}

#[test]
fn a_failed_command_and_a_command_that_wrote_nothing_are_notes() {
    let mut fact = checked();
    fact.run.status = RunStatus::Failed;
    fact.run.detail = Some("exit status 1".to_owned());
    let (result, findings) = support::run_over(ID, fact);
    assert_eq!(findings.len(), 2);
    assert_eq!(
        result.notes[0],
        "the history command `go test -run TestHistory ./...` failed: exit status 1"
    );
    let empty = Histories {
        histories: Vec::new(),
        ..checked()
    };
    let (result, findings) = support::run_over(ID, empty);
    assert!(findings.is_empty());
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.score, Some(score(0.0, "violations", 0)));
    assert_eq!(
        result.notes,
        ["the history command `go test -run TestHistory ./...` wrote no history"]
    );
}

#[test]
fn settings_are_refused() {
    support::assert_refuses_settings(ID);
}

#[test]
fn every_metric_adds_up_across_languages() {
    support::assert_counts_add_up(
        ID,
        &["histories", "operations", "violations", "unknown", "invalid"],
        "violations",
    );
}

#[test]
fn the_manifest_reads_the_histories() {
    assert_eq!(
        Shape::of(support::check(ID).manifest()),
        Shape {
            needs: vec![FactKind::Histories],
            wants: vec![],
            tier: 5,
            cost: CostClass::Deep,
            kinds: vec![("consistency.violation", ActionKind::FixBug)],
            metrics: ["histories", "operations", "violations", "unknown", "invalid"]
                .map(|metric| (metric, MetricPer::Scope))
                .to_vec(),
            evidence: vec!["object"],
        }
    );
}
