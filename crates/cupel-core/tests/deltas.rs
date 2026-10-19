// ABOUTME: Tests for `deltas`: comparing two runs' findings by ID and across symbol matches, check scores and
// ABOUTME: per-symbol metrics, and applying the result to an envelope (ADR-046, ADR-131).
use std::collections::BTreeSet;
use std::str::FromStr;

use cupel_core::matching::{Matched, SymbolMatch};
use cupel_core::syntax::SymbolKey;
use cupel_core::{
    BaselineRef, CheckId, Delta, Envelope, FindingId, LangId, Matching, RunId, Symbol,
    SymbolMetrics, deltas, deltas_matched,
};
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
        "cupel": {"version": "0.1.0", "schema": "https://tysont.github.io/cupel/schema/v1/run.json"},
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

// ---------------------------------------------------------------- across symbol matches

/// A finding of `kind` about the symbol `hash`.
fn on_symbol(id: &str, kind: &str, hash: &str) -> Value {
    let mut finding = finding(id);
    finding["kind"] = json!(kind);
    finding["symbol"] = json!({"display": hash, "hash": hash, "lang": "rust"});
    finding
}

fn with_findings(run: &str, findings: Vec<Value>, checks: &[Value]) -> Envelope {
    let mut envelope = envelope(run, &[], checks);
    envelope.findings = findings
        .into_iter()
        .map(|finding| serde_json::from_value(finding).unwrap())
        .collect();
    envelope
}

fn member(hash: &str, nodes: u32) -> Matched {
    Matched {
        key: SymbolKey {
            file: "src/lib.rs".to_owned(),
            line: nodes,
            col: 1,
        },
        symbol: Symbol {
            display: hash.to_owned(),
            hash: hash.parse().unwrap(),
            lang: LangId::from("rust"),
        },
        nodes,
    }
}

const OLD: &str = "sh_0000000000000001";
const LEFT: &str = "sh_0000000000000002";
const RIGHT: &str = "sh_0000000000000003";

/// The reference's OLD split into LEFT and RIGHT.
fn split() -> Vec<SymbolMatch> {
    vec![SymbolMatch {
        base: vec![member(OLD, 30)],
        head: vec![member(LEFT, 40), member(RIGHT, 20)],
        similarity: 0.8,
    }]
}

#[test]
fn a_finding_carried_across_a_match_is_neither_new_nor_resolved() {
    let against = with_findings(
        A,
        vec![on_symbol("fnd_000001", "complexity.high", OLD)],
        &[],
    );
    let current = with_findings(
        B,
        vec![on_symbol("fnd_000002", "complexity.high", LEFT)],
        &[],
    );
    let matches = split();
    let matching = Matching {
        matches: &matches,
        ..Matching::default()
    };

    let deltas = deltas_matched(&current, &against, &matching);

    assert_eq!(deltas.new, Vec::<FindingId>::new());
    assert_eq!(ids(&deltas.resolved), Vec::<String>::new());
    let sets = &deltas.sets[&CheckId::from("static.complexity")];
    assert_eq!(
        serde_json::to_value(sets).unwrap(),
        json!([{
            "base": [{"display": OLD, "hash": OLD, "lang": "rust"}],
            "head": [
                {"display": LEFT, "hash": LEFT, "lang": "rust"},
                {"display": RIGHT, "hash": RIGHT, "lang": "rust"}
            ],
            "similarity": 0.8,
            "findings": {"base": ["fnd_000001"], "head": ["fnd_000002"]},
            "metrics": {}
        }])
    );
}

#[test]
fn findings_of_another_kind_or_outside_the_match_stay_new_and_resolved() {
    let against = with_findings(
        A,
        vec![
            on_symbol("fnd_000001", "complexity.high", OLD),
            on_symbol("fnd_000003", "complexity.high", "sh_00000000000000ff"),
        ],
        &[],
    );
    let current = with_findings(
        B,
        vec![
            on_symbol("fnd_000002", "nesting.deep", LEFT),
            on_symbol("fnd_000004", "complexity.high", "sh_00000000000000fe"),
        ],
        &[],
    );
    let matches = split();
    let matching = Matching {
        matches: &matches,
        ..Matching::default()
    };

    let deltas = deltas_matched(&current, &against, &matching);

    let new: Vec<String> = deltas.new.iter().map(FindingId::to_string).collect();
    assert_eq!(new, ["fnd_000002", "fnd_000004"]);
    assert_eq!(ids(&deltas.resolved), ["fnd_000001", "fnd_000003"]);
    assert!(deltas.sets.is_empty());
}

#[test]
fn per_symbol_metrics_compare_as_sums_and_scores_as_means_weighted_by_tree_size() {
    let check = CheckId::from("static.complexity");
    let against = envelope(A, &[], &[]);
    let current = envelope(B, &[], &[]);
    let mut reference = SymbolMetrics::default();
    reference.insert(&check, "cyclomatic", OLD.parse().unwrap(), 12.0);
    reference.insert(&check, "crap", OLD.parse().unwrap(), 30.0);
    let mut now = SymbolMetrics::default();
    now.insert(&check, "cyclomatic", LEFT.parse().unwrap(), 7.0);
    now.insert(&check, "cyclomatic", RIGHT.parse().unwrap(), 6.0);
    now.insert(&check, "crap", LEFT.parse().unwrap(), 10.0);
    now.insert(&check, "crap", RIGHT.parse().unwrap(), 40.0);
    // Measured only at head: nothing to compare against.
    now.insert(&check, "cognitive", LEFT.parse().unwrap(), 3.0);
    let matches = split();
    let matching = Matching {
        matches: &matches,
        current: now,
        reference,
        means: BTreeSet::from([(check.clone(), "crap".to_owned())]),
    };

    let deltas = deltas_matched(&current, &against, &matching);

    assert_eq!(
        serde_json::to_value(&deltas.sets[&check][0].metrics).unwrap(),
        // crap: (10 × 40 + 40 × 20) / (40 + 20).
        json!({"crap": {"base": 30.0, "head": 20.0}, "cyclomatic": {"base": 12.0, "head": 13.0}})
    );
}

#[test]
fn applying_puts_each_checks_sets_on_its_delta() {
    let complexity = check("static.complexity", Some(10.0));
    let against = with_findings(
        A,
        vec![on_symbol("fnd_000001", "complexity.high", OLD)],
        std::slice::from_ref(&complexity),
    );
    let mut current = with_findings(
        B,
        vec![on_symbol("fnd_000002", "complexity.high", RIGHT)],
        &[complexity],
    );
    let matches = split();
    let matching = Matching {
        matches: &matches,
        ..Matching::default()
    };

    deltas_matched(&current, &against, &matching).apply(
        &mut current,
        BaselineRef {
            id: A.parse().unwrap(),
            name: "main".to_owned(),
        },
        "main",
    );

    assert!(!current.findings[0].new_since_baseline);
    assert_eq!((current.summary.new, current.summary.resolved), (0, 0));
    let delta = current.checks[0].delta.as_ref().unwrap();
    assert_eq!(delta.sets.len(), 1);
    assert_eq!(
        delta.sets[0].findings.head,
        [FindingId::from_str("fnd_000002").unwrap()]
    );
}

#[test]
fn a_delta_without_sets_serializes_as_before() {
    let delta = Delta {
        vs: "baseline".to_owned(),
        value: 0.5,
        sets: Vec::new(),
    };
    assert_eq!(
        serde_json::to_value(delta).unwrap(),
        json!({"vs": "baseline", "value": 0.5})
    );
}
