// ABOUTME: spec.prove over synthetic ProofResults facts: a failed obligation is a failing finding, an open one a warning,
// ABOUTME: both at their places; failed runs and no proofs are notes; the score; the manifest (ADR-118).
mod support;

use cupel_core::{
    ActionKind, CheckId, CheckStatus, CostClass, FactKind, FindingId, Location, MetricPer,
    Obligation, ObligationStatus, ProducerId, ProofResults, RelatedPlace, RootRun, RunStatus,
    Severity,
};
use serde_json::json;
use support::{Shape, score};

const ID: &str = "spec.prove";

fn at(line: u32) -> Location {
    Location {
        file: "proofs/tally.dfy".to_owned(),
        line,
        col: Some(3),
        end_line: line,
        end_col: Some(9),
    }
}

fn obligation(status: ObligationStatus, line: u32, message: &str) -> Obligation {
    Obligation {
        status,
        message: message.to_owned(),
        location: at(line),
        related: Vec::new(),
    }
}

fn verified(obligations: Vec<Obligation>) -> ProofResults {
    ProofResults {
        tool: ProducerId::from("tool.dafny"),
        runs: vec![RootRun {
            dir: ".".to_owned(),
            status: RunStatus::Ran,
            detail: None,
        }],
        files: vec!["proofs/tally.dfy".to_owned()],
        verified: 3,
        obligations,
    }
}

fn unproved() -> Obligation {
    Obligation {
        related: vec![RelatedPlace {
            message: "this is the postcondition that could not be proved".to_owned(),
            location: at(23),
        }],
        ..obligation(
            ObligationStatus::Failed,
            24,
            "a postcondition could not be proved on this return path",
        )
    }
}

#[test]
fn a_failed_obligation_fails_the_check_at_its_place_with_its_related_places() {
    let (result, findings) = support::run_over(ID, verified(vec![unproved()]));
    assert_eq!(result.status, CheckStatus::Fail);
    let [finding] = findings.as_slice() else {
        panic!("{findings:?}")
    };
    assert_eq!(finding.kind, "proof.failed");
    assert_eq!(
        (finding.severity, finding.action.kind, finding.confidence),
        (Severity::Fail, ActionKind::FixBug, 1.0)
    );
    assert_eq!(finding.location, Some(at(24)));
    assert_eq!(
        finding.title,
        "proof fails at proofs/tally.dfy:24: a postcondition could not be proved on this return \
         path"
    );
    assert_eq!(
        finding.evidence.0,
        *json!({
            "tool": "tool.dafny",
            "message": "a postcondition could not be proved on this return path",
            "related": [{
                "message": "this is the postcondition that could not be proved",
                "file": "proofs/tally.dfy",
                "line": 23,
            }],
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(
        finding.id,
        FindingId::for_subject(
            &CheckId::from(ID),
            "proof.failed",
            "proofs/tally.dfy|a postcondition could not be proved on this return path|1"
        )
    );
    assert_eq!(
        finding.repro,
        "cupel check --only spec.prove --paths proofs/tally.dfy"
    );
}

#[test]
fn an_open_obligation_is_a_warning_and_repeats_are_told_apart() {
    let assumed = |line| {
        obligation(
            ObligationStatus::Open,
            line,
            "assume statement has no {:axiom} annotation",
        )
    };
    let (result, findings) = support::run_over(ID, verified(vec![assumed(31), assumed(40)]));
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(
        findings
            .iter()
            .map(|finding| (finding.kind.as_str(), finding.severity, finding.action.kind))
            .collect::<Vec<_>>(),
        [("proof.obligation-open", Severity::Warn, ActionKind::Review); 2]
    );
    assert_eq!(
        findings[0].title,
        "proof obligation open at proofs/tally.dfy:31: assume statement has no {:axiom} \
         annotation"
    );
    assert_ne!(findings[0].id, findings[1].id);
}

#[test]
fn the_score_counts_undischarged_obligations_and_failed_runs_are_notes() {
    let mut results = verified(vec![
        unproved(),
        obligation(
            ObligationStatus::Open,
            31,
            "assume statement has no {:axiom} annotation",
        ),
    ]);
    results.runs.push(RootRun {
        dir: "tools".to_owned(),
        status: RunStatus::Failed,
        detail: Some("tools/a.dfy:5:36: unresolved identifier: Summ".to_owned()),
    });
    let (result, _) = support::run_over(ID, results);
    assert_eq!(result.score, Some(score(2.0, "undischarged", 1)));
    assert_eq!(
        result.metrics,
        [
            ("failed".to_owned(), 1.0),
            ("open".to_owned(), 1.0),
            ("verified".to_owned(), 3.0),
        ]
        .into()
    );
    assert_eq!(
        result.notes,
        ["tool.dafny failed in tools: tools/a.dfy:5:36: unresolved identifier: Summ"]
    );
}

#[test]
fn proofs_that_all_verify_pass_and_no_proof_file_scores_nothing() {
    let (result, findings) = support::run_over(ID, verified(Vec::new()));
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.score, Some(score(0.0, "undischarged", 1)));

    let (none, _) = support::run_over(
        ID,
        ProofResults {
            runs: Vec::new(),
            files: Vec::new(),
            verified: 0,
            ..verified(Vec::new())
        },
    );
    assert_eq!(none.score, None);
    assert!(none.metrics.is_empty(), "{:?}", none.metrics);
    assert_eq!(
        none.notes,
        ["tool.dafny found no proof file in the project roots"]
    );
}

#[test]
fn settings_are_refused() {
    support::assert_refuses_settings(ID);
}

#[test]
fn every_metric_adds_up_across_languages() {
    support::assert_counts_add_up(ID, &["verified", "failed", "open"], "undischarged");
}

#[test]
fn the_manifest_reads_the_proof_results() {
    assert_eq!(
        Shape::of(support::check(ID).manifest()),
        Shape {
            needs: vec![FactKind::ProofResults],
            wants: vec![],
            tier: 6,
            cost: CostClass::Exhaustive,
            kinds: vec![
                ("proof.failed", ActionKind::FixBug),
                ("proof.obligation-open", ActionKind::Review),
            ],
            metrics: ["verified", "failed", "open"]
                .map(|metric| (metric, MetricPer::Scope))
                .to_vec(),
            evidence: vec!["object", "object"],
        }
    );
}
