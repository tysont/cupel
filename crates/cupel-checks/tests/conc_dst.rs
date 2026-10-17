// ABOUTME: conc.dst over synthetic SimulationResults facts: a failing seed with its trace and the repro that reruns it
// ABOUTME: alone, a failure without a seed, notes for failed runs and roots without a harness, the manifest (ADR-120).
mod support;

use cupel_core::{
    ActionKind, CheckId, CheckStatus, CostClass, FactKind, FindingId, Location, MetricPer,
    ProducerId, RootRun, RunStatus, Severity, SimulationFailure, SimulationHarness,
    SimulationResults, SimulationStatus,
};
use serde_json::json;
use support::{Shape, score};

const ID: &str = "conc.dst";

fn at(file: &str, line: u32) -> Location {
    Location {
        file: file.to_owned(),
        line,
        col: Some(5),
        end_line: line,
        end_col: None,
    }
}

fn harness(name: &str, status: SimulationStatus) -> SimulationHarness {
    SimulationHarness {
        root: ".".to_owned(),
        name: name.to_owned(),
        location: Some(at("tests/sim.rs", 8)),
        status,
        failure: None,
    }
}

fn lost_deposit() -> SimulationFailure {
    SimulationFailure {
        seed: Some(4),
        message: "assertion `left == right` failed: a deposit was lost".to_owned(),
        location: Some(at("tests/sim.rs", 23)),
        trace: vec![
            "deposit 10 reads 0 at 3ms".to_owned(),
            "deposit 20 reads 0 at 5ms".to_owned(),
        ],
        reproduce: "RUSTFLAGS='--cfg madsim' MADSIM_TEST_SEED=4 cargo test --test sim \
                    concurrent_deposits_are_all_kept -- --exact"
            .to_owned(),
    }
}

/// One root with a harness failing at seed 4, one failing with no seed, and one passing; a
/// second root whose run failed.
fn campaign() -> SimulationResults {
    let mut failed = harness("concurrent_deposits_are_all_kept", SimulationStatus::Failed);
    failed.failure = Some(lost_deposit());
    let mut unseeded = harness("sim_setup", SimulationStatus::Failed);
    unseeded.location = None;
    unseeded.failure = Some(SimulationFailure {
        seed: None,
        location: None,
        message: "could not bind the simulated address".to_owned(),
        trace: Vec::new(),
        reproduce: "RUSTFLAGS='--cfg madsim' cargo test --test sim sim_setup -- --exact".to_owned(),
    });
    SimulationResults {
        tool: ProducerId::from("tool.madsim"),
        seed: 3,
        seeds: 1000,
        runs: vec![
            RootRun {
                dir: ".".to_owned(),
                status: RunStatus::Ran,
                detail: None,
            },
            RootRun {
                dir: "tools".to_owned(),
                status: RunStatus::Failed,
                detail: Some("error[E0425]: cannot find value `x`".to_owned()),
            },
        ],
        harnesses: vec![
            failed,
            unseeded,
            harness("sequential_deposits_are_all_kept", SimulationStatus::Passed),
        ],
    }
}

#[test]
fn a_failing_seed_is_a_failing_finding_whose_repro_reruns_that_seed_alone() {
    let (_, findings) = support::run_over(ID, campaign());
    assert_eq!(findings.len(), 2, "{findings:#?}");
    let failure = &findings[0];
    assert_eq!(failure.kind, "dst.failure");
    assert_eq!(
        (failure.severity, failure.action.kind, failure.confidence),
        (Severity::Fail, ActionKind::FixBug, 1.0)
    );
    assert_eq!(
        failure.id,
        FindingId::for_subject(
            &CheckId::from(ID),
            "dst.failure",
            ".|concurrent_deposits_are_all_kept"
        )
    );
    assert_eq!(failure.location, Some(at("tests/sim.rs", 23)));
    assert_eq!(
        failure.title,
        "simulation `concurrent_deposits_are_all_kept` fails at seed 4: assertion `left == \
         right` failed: a deposit was lost"
    );
    assert_eq!(
        failure.evidence.0,
        *json!({
            "harness": "concurrent_deposits_are_all_kept",
            "tool": "tool.madsim",
            "seed": 4,
            "first_seed": 3,
            "seeds": 1000,
            "message": "assertion `left == right` failed: a deposit was lost",
            "trace": ["deposit 10 reads 0 at 3ms", "deposit 20 reads 0 at 5ms"],
            "reproduce": lost_deposit().reproduce,
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(
        failure.repro,
        "cupel check --only conc.dst --paths tests/sim.rs --seed 4 --cases 1"
    );
    assert!(
        failure.action.summary.contains(&lost_deposit().reproduce),
        "{}",
        failure.action.summary
    );
}

#[test]
fn a_failure_without_a_seed_reruns_the_whole_range_from_the_root() {
    let (_, findings) = support::run_over(ID, campaign());
    let unseeded = &findings[1];
    assert_eq!(unseeded.location, None);
    assert_eq!(
        unseeded.title,
        "simulation `sim_setup` fails: could not bind the simulated address"
    );
    assert_eq!(unseeded.evidence.0["seed"], json!(null));
    assert_eq!(
        unseeded.repro,
        "cupel check --only conc.dst --paths . --seed 3 --cases 1000"
    );
}

#[test]
fn the_score_counts_failing_harnesses_and_failed_runs_are_notes() {
    let (result, _) = support::run_over(ID, campaign());
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.score, Some(score(2.0, "failing", 3)));
    assert_eq!(
        result.metrics,
        [("failures".to_owned(), 2.0), ("harnesses".to_owned(), 3.0),].into()
    );
    assert_eq!(
        result.notes,
        ["tool.madsim failed in tools: error[E0425]: cannot find value `x`"]
    );
}

#[test]
fn a_root_without_a_harness_is_a_note_and_passing_harnesses_pass() {
    let results = SimulationResults {
        runs: vec![RootRun {
            dir: ".".to_owned(),
            status: RunStatus::Ran,
            detail: None,
        }],
        harnesses: Vec::new(),
        ..campaign()
    };
    let (result, findings) = support::run_over(ID, results);
    assert!(findings.is_empty());
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.score, Some(score(0.0, "failing", 0)));
    assert_eq!(
        result.notes,
        ["tool.madsim found no simulation harness in ."]
    );
    let passing = SimulationResults {
        harnesses: vec![harness("fine", SimulationStatus::Passed)],
        ..campaign()
    };
    let (result, findings) = support::run_over(ID, passing);
    assert!(findings.is_empty());
    assert_eq!(result.status, CheckStatus::Pass);
}

#[test]
fn settings_are_refused() {
    support::assert_refuses_settings(ID);
}

#[test]
fn every_count_adds_up_across_languages() {
    support::assert_counts_add_up(ID, &["harnesses", "failures"], "failing");
}

#[test]
fn the_manifest_reads_the_simulation_results() {
    assert_eq!(
        Shape::of(support::check(ID).manifest()),
        Shape {
            needs: vec![FactKind::SimulationResults],
            wants: vec![],
            tier: 4,
            cost: CostClass::Exhaustive,
            kinds: vec![("dst.failure", ActionKind::FixBug)],
            metrics: ["harnesses", "failures"]
                .map(|metric| (metric, MetricPer::Scope))
                .to_vec(),
            evidence: vec!["object"],
        }
    );
}
