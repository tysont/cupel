// ABOUTME: The file copies' unmutated baseline (ADR-185): it runs once per production in the first slot, is stored by
// ABOUTME: suite hash, and a later production reuses it until a test, a source, or the runner's command changes.
mod support;

use cupel_core::{MutationOutcomes, MutationPhase, PhaseDuration};
use support::{Run, produce};

/// The rows of the `build` phase.
fn builds(outcomes: &MutationOutcomes) -> Vec<&PhaseDuration> {
    outcomes
        .phases
        .iter()
        .filter(|row| row.phase == MutationPhase::Build)
        .collect()
}

/// The Python fixture's baseline, as `(file, slot, tests)` of each `build` row.
fn shape(outcomes: &MutationOutcomes) -> Vec<(Option<String>, Option<u32>, Option<u32>)> {
    builds(outcomes)
        .iter()
        .map(|row| (row.file.clone(), row.slot, row.tests))
        .collect()
}

#[test]
fn a_file_copy_production_runs_its_baseline_once_in_the_first_slot_over_the_whole_suite() {
    let Some((_dir, outcomes)) = support::python() else {
        return;
    };
    assert_eq!(
        shape(&outcomes),
        [(None, Some(0), None)],
        "{:?}",
        outcomes.phases
    );
}

#[test]
fn a_later_production_reuses_the_stored_baseline_while_the_suite_is_unchanged() {
    let Some((dir, first)) = support::python() else {
        return;
    };
    let second = produce(dir.path(), "python", &Run::default()).unwrap();
    assert!(builds(&second).is_empty(), "{:?}", second.phases);
    assert_eq!(
        support::summary(&second),
        support::summary(&first),
        "the stored baseline's timeouts find what a measured one finds"
    );
}

#[test]
fn a_changed_source_or_test_file_reruns_the_baseline() {
    let Some((dir, _)) = support::python() else {
        return;
    };
    let source = dir.path().join("discount.py");
    let text = std::fs::read_to_string(&source).unwrap();
    std::fs::write(&source, text.replace("total - 5", "total - 6")).unwrap();
    let changed = produce(dir.path(), "python", &Run::default()).unwrap();
    assert_eq!(builds(&changed).len(), 1, "{:?}", changed.phases);
    let test_file = dir.path().join("tests/test_small.py");
    let text = std::fs::read_to_string(&test_file).unwrap();
    std::fs::write(&test_file, format!("{text}\n# The boundary is untested.\n")).unwrap();
    let retested = produce(dir.path(), "python", &Run::default()).unwrap();
    assert_eq!(builds(&retested).len(), 1, "{:?}", retested.phases);
    let again = produce(dir.path(), "python", &Run::default()).unwrap();
    assert!(builds(&again).is_empty(), "{:?}", again.phases);
}
