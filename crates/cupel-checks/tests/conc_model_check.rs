// ABOUTME: conc.model-check over synthetic ModelCheckResults facts: a refuted property is a failing finding with its
// ABOUTME: counterexample, undetermined harnesses and failed runs are notes, the score, the manifest (ADR-117).
mod support;

use cupel_core::{
    ActionKind, CheckId, CheckStatus, CostClass, Counterexample, FactKind, FindingId,
    HarnessStatus, Location, MetricPer, ModelCheckResults, ModelHarness, ProducerId,
    RefutedProperty, RootRun, RunStatus, Severity,
};
use serde_json::json;
use support::{Shape, score};

const ID: &str = "conc.model-check";

fn at(line: u32) -> Location {
    Location {
        file: "src/lib.rs".to_owned(),
        line,
        col: None,
        end_line: line,
        end_col: None,
    }
}

fn harness(name: &str, status: HarnessStatus, refuted: Vec<RefutedProperty>) -> ModelHarness {
    ModelHarness {
        root: ".".to_owned(),
        name: name.to_owned(),
        location: Some(at(21)),
        status,
        refuted,
    }
}

fn overflow(location: Option<Location>, values: &[&str]) -> RefutedProperty {
    RefutedProperty {
        description: "attempt to add with overflow".to_owned(),
        function: Some("average".to_owned()),
        location,
        counterexample: (!values.is_empty()).then(|| Counterexample {
            values: values.iter().map(|value| (*value).to_owned()).collect(),
            replay: Some("#[test]\nfn replay() {}\n".to_owned()),
        }),
    }
}

fn checked(harnesses: Vec<ModelHarness>) -> ModelCheckResults {
    ModelCheckResults {
        tool: ProducerId::from("tool.kani"),
        runs: vec![RootRun {
            dir: ".".to_owned(),
            status: RunStatus::Ran,
            detail: None,
        }],
        harnesses,
    }
}

#[test]
fn a_refuted_property_is_a_failing_finding_with_its_counterexample() {
    let (result, findings) = support::run_over(
        ID,
        checked(vec![
            harness(
                "proofs::average_never_overflows",
                HarnessStatus::Failed,
                vec![overflow(Some(at(3)), &["128", "128"])],
            ),
            harness(
                "proofs::midpoint_lies_between",
                HarnessStatus::Verified,
                vec![],
            ),
        ]),
    );
    assert_eq!(result.status, CheckStatus::Fail);
    let [finding] = findings.as_slice() else {
        panic!("{findings:?}")
    };
    assert_eq!(finding.kind, "model.counterexample");
    assert_eq!(
        (finding.severity, finding.action.kind, finding.confidence),
        (Severity::Fail, ActionKind::FixBug, 1.0)
    );
    assert_eq!(finding.location, Some(at(3)));
    assert_eq!(
        finding.title,
        "harness `proofs::average_never_overflows` refutes `attempt to add with overflow` in \
         `average` with 128, 128"
    );
    assert_eq!(
        finding.evidence.0,
        *json!({
            "tool": "tool.kani",
            "harness": "proofs::average_never_overflows",
            "description": "attempt to add with overflow",
            "function": "average",
            "values": ["128", "128"],
            "replay": "#[test]\nfn replay() {}\n",
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(
        finding.id,
        FindingId::for_subject(
            &CheckId::from(ID),
            "model.counterexample",
            ".|proofs::average_never_overflows|attempt to add with overflow"
        )
    );
    assert_eq!(
        finding.repro,
        "cupel check --only conc.model-check --paths src/lib.rs"
    );
}

#[test]
fn a_property_refuted_outside_the_repository_is_placed_at_its_harness() {
    let (_, findings) = support::run_over(
        ID,
        checked(vec![harness(
            "proofs::average_never_overflows",
            HarnessStatus::Failed,
            vec![overflow(None, &[])],
        )]),
    );
    assert_eq!(findings[0].location, Some(at(21)));
    assert_eq!(
        findings[0].title,
        "harness `proofs::average_never_overflows` refutes `attempt to add with overflow` in \
         `average`"
    );
    assert_eq!(findings[0].evidence.0["values"], json!([]));
    assert_eq!(findings[0].evidence.0["replay"], json!(null));
}

#[test]
fn the_score_counts_failing_harnesses_and_the_undecided_are_notes() {
    let mut results = checked(vec![
        harness(
            "proofs::average_never_overflows",
            HarnessStatus::Failed,
            vec![overflow(Some(at(3)), &["128", "128"])],
        ),
        harness(
            "proofs::midpoint_lies_between",
            HarnessStatus::Verified,
            vec![],
        ),
        harness("proofs::spins", HarnessStatus::Undetermined, vec![]),
    ]);
    results.runs.push(RootRun {
        dir: "tools".to_owned(),
        status: RunStatus::Failed,
        detail: Some("error[E0277]: cannot divide `u8` by `&str`".to_owned()),
    });
    let (result, _) = support::run_over(ID, results);
    assert_eq!(result.score, Some(score(1.0, "failing", 3)));
    assert_eq!(
        result.metrics,
        [
            ("failed".to_owned(), 1.0),
            ("harnesses".to_owned(), 3.0),
            ("undetermined".to_owned(), 1.0),
            ("verified".to_owned(), 1.0),
        ]
        .into()
    );
    assert_eq!(
        result.notes,
        [
            "tool.kani failed in tools: error[E0277]: cannot divide `u8` by `&str`",
            "harness `proofs::spins` is undetermined: the model checker could not decide it",
        ]
    );
}

#[test]
fn verified_harnesses_pass_and_no_run_scores_nothing() {
    let (result, findings) = support::run_over(
        ID,
        checked(vec![harness(
            "proofs::midpoint_lies_between",
            HarnessStatus::Verified,
            vec![],
        )]),
    );
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.score, Some(score(0.0, "failing", 1)));

    let (unrun, _) = support::run_over(
        ID,
        ModelCheckResults {
            runs: Vec::new(),
            ..checked(Vec::new())
        },
    );
    assert_eq!(unrun.score, None);
    assert!(unrun.metrics.is_empty(), "{:?}", unrun.metrics);
}

#[test]
fn settings_are_refused() {
    support::assert_refuses_settings(ID);
}

#[test]
fn every_metric_adds_up_across_languages() {
    support::assert_counts_add_up(
        ID,
        &["harnesses", "verified", "failed", "undetermined"],
        "failing",
    );
}

#[test]
fn the_manifest_reads_the_model_check_results() {
    assert_eq!(
        Shape::of(support::check(ID).manifest()),
        Shape {
            needs: vec![FactKind::ModelCheckResults],
            wants: vec![],
            tier: 5,
            cost: CostClass::Exhaustive,
            kinds: vec![("model.counterexample", ActionKind::FixBug)],
            metrics: ["harnesses", "verified", "failed", "undetermined"]
                .map(|metric| (metric, MetricPer::Scope))
                .to_vec(),
            evidence: vec!["object"],
        }
    );
}
