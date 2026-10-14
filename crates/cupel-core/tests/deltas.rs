// ABOUTME: Tests for `deltas`: comparing two runs' findings by ID and check scores, and applying
// ABOUTME: the result to an envelope's findings, summary, checks, and `run.baseline` (ADR-046).
use cupel_core::{BaselineRef, CheckId, Envelope, FindingId, RunId, deltas};
use serde_json::{Value, json};

fn finding(id: &str) -> Value {
    json!({
        "id": id, "check": "static.complexity", "kind": "complexity.high",
        "severity": "warn", "confidence": 1.0, "tier": 1,
        "symbol": null, "location": null, "title": format!("finding {id}"), "detail": null,
        "evidence": {}, "action": {"kind": "review", "priority": 1, "summary": "Look.", "hints": {}},
        "repro": "cupel check", "suppress": format!("cupel suppress {id} --reason '…'"),
        "new_since_baseline": false, "suppressed": false
    })
}

fn check(id: &str, score: Option<f64>) -> Value {
    json!({
        "id": id, "status": "pass", "tier": 1, "cost": "quick",
        "score": score.map(|value| json!({"value": value, "unit": "ratio", "n": 3})),
        "delta": null, "findings": 0, "duration_ms": 5, "skipped": null, "oracle": null
    })
}

fn envelope(run: &str, findings: &[&str], checks: &[Value]) -> Envelope {
    let findings: Vec<Value> = findings.iter().map(|id| finding(id)).collect();
    serde_json::from_value(json!({
        "cupel": {"version": "0.1.0", "schema": "https://cupel.dev/schema/v1/run.json"},
        "run": {
            "id": run, "verb": "check", "started": "2026-10-13T10:00:00Z", "duration_ms": 1,
            "git": null, "scope": null, "depth": "standard", "budget_ms": null, "seed": 1,
            "toolchain": {}, "config": {"effective": {}, "sources": []}, "baseline": null
        },
        "status": "pass", "exit_code": 0, "checks": checks,
        "total_findings": findings.len(),
        "findings": findings,
        "summary": {"findings": {"fail": 0, "warn": 0, "info": 0}, "new": 0, "resolved": 0,
                    "suppressed": 0, "gate": null},
        "next_actions": [], "truncated": false
    }))
    .unwrap()
}

const A: &str = "run_01JA2B3C4D5E6F7G8H9JKMNPQA";
const B: &str = "run_01JA2B3C4D5E6F7G8H9JKMNPQB";

fn ids(findings: &[cupel_core::Finding]) -> Vec<String> {
    findings.iter().map(|f| f.id.to_string()).collect()
}

#[test]
fn new_and_resolved_are_set_differences_by_finding_id() {
    let against = envelope(A, &["fnd_000001", "fnd_000002"], &[]);
    let current = envelope(B, &["fnd_000002", "fnd_000003", "fnd_000004"], &[]);

    let deltas = deltas(&current, &against);

    let new: Vec<String> = deltas.new.iter().map(FindingId::to_string).collect();
    assert_eq!(new, ["fnd_000003", "fnd_000004"]);
    assert_eq!(ids(&deltas.resolved), ["fnd_000001"]);
}

#[test]
fn applying_marks_new_findings_counts_and_names_the_reference() {
    let against = envelope(A, &["fnd_000001", "fnd_000002"], &[]);
    let mut current = envelope(B, &["fnd_000002", "fnd_000003"], &[]);
    let reference = BaselineRef {
        id: A.parse::<RunId>().unwrap(),
        name: "default".to_owned(),
    };

    deltas(&current, &against).apply(&mut current, reference.clone(), "baseline");

    let marks: Vec<bool> = current
        .findings
        .iter()
        .map(|f| f.new_since_baseline)
        .collect();
    assert_eq!(marks, [false, true]);
    assert_eq!(current.summary.new, 1);
    assert_eq!(current.summary.resolved, 1);
    assert_eq!(current.run.baseline, Some(reference));
}

#[test]
fn check_deltas_are_score_differences_for_checks_scored_in_both_runs() {
    let against = envelope(
        A,
        &[],
        &[
            check("static.complexity", Some(0.5)),
            check("static.crap", Some(0.9)),
            check("static.clones", None),
        ],
    );
    let mut current = envelope(
        B,
        &[],
        &[
            check("static.complexity", Some(0.75)),
            check("static.crap", None),
            check("static.clones", Some(0.1)),
            check("static.deps", Some(1.0)),
        ],
    );

    deltas(&current, &against).apply(
        &mut current,
        BaselineRef {
            id: A.parse().unwrap(),
            name: "run:x".to_owned(),
        },
        &format!("run:{A}"),
    );

    let delta = |id: &str| {
        current
            .checks
            .iter()
            .find(|c| c.id == CheckId::from(id))
            .unwrap()
            .delta
            .clone()
    };
    let complexity = delta("static.complexity").unwrap();
    assert_eq!(complexity.vs, format!("run:{A}"));
    assert!((complexity.value - 0.25).abs() < 1e-9);
    assert_eq!(delta("static.crap"), None);
    assert_eq!(delta("static.clones"), None);
    assert_eq!(delta("static.deps"), None);
}

#[test]
fn identical_runs_have_no_deltas() {
    let against = envelope(A, &["fnd_000001"], &[]);
    let current = envelope(B, &["fnd_000001"], &[]);
    let deltas = deltas(&current, &against);
    assert!(deltas.new.is_empty());
    assert!(deltas.resolved.is_empty());
}
