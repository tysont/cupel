// ABOUTME: The engine's phase breakdown (ADR-150): a production's `phases` are its timeline, one plan, a sync per slot,
// ABOUTME: a build per slot per file (with the tests it selected), a rewrite per schemata file, and a run per mutant.
mod support;

use std::collections::BTreeSet;

use cupel_core::{MutantOutcome, MutationOutcomes, MutationPhase, PhaseDuration};
use support::{Run, fixture, produce};

/// The rows of `phase`.
fn of(outcomes: &MutationOutcomes, phase: MutationPhase) -> Vec<&PhaseDuration> {
    outcomes
        .phases
        .iter()
        .filter(|row| row.phase == phase)
        .collect()
}

/// The slots the rows of `phase` ran in.
fn slots(outcomes: &MutationOutcomes, phase: MutationPhase) -> BTreeSet<Option<u32>> {
    of(outcomes, phase).iter().map(|row| row.slot).collect()
}

/// The mutants that ran this time, each as its file and run time.
fn ran(outcomes: &MutationOutcomes) -> Vec<(String, u64)> {
    outcomes
        .mutants
        .iter()
        .filter(|mutant| !mutant.reused && mutant.outcome != MutantOutcome::Uncovered)
        .map(|mutant| (mutant.symbol.file.clone(), mutant.duration_ms.unwrap()))
        .collect()
}

#[test]
fn a_file_copy_production_plans_once_syncs_each_slot_builds_in_each_and_runs_each_mutant() {
    let Some((_dir, outcomes)) = support::python() else {
        return;
    };
    let plan = of(&outcomes, MutationPhase::Plan);
    assert_eq!(plan.len(), 1, "{:?}", outcomes.phases);
    assert_eq!((plan[0].file.as_deref(), plan[0].slot), (None, None));
    assert_eq!(
        slots(&outcomes, MutationPhase::Sync),
        slots(&outcomes, MutationPhase::Build),
        "every slot the file's build used was synced first"
    );
    assert!(
        of(&outcomes, MutationPhase::Sync)
            .iter()
            .all(|row| row.file.is_none() && row.slot.is_some()),
        "{:?}",
        outcomes.phases
    );
    // Without a per-test map every mutant runs the whole suite: no test count on the builds.
    assert!(
        of(&outcomes, MutationPhase::Build)
            .iter()
            .all(|row| row.file.as_deref() == Some("discount.py") && row.tests.is_none()),
        "{:?}",
        outcomes.phases
    );
    assert!(of(&outcomes, MutationPhase::Rewrite).is_empty());
    let mut runs: Vec<(String, u64)> = of(&outcomes, MutationPhase::Run)
        .iter()
        .map(|row| (row.file.clone().unwrap(), row.duration_ms))
        .collect();
    let mut mutants = ran(&outcomes);
    runs.sort_unstable();
    mutants.sort_unstable();
    assert_eq!(runs, mutants, "one run per executed mutant, timed alike");
}

#[test]
fn with_a_per_test_map_the_build_counts_the_tests_the_files_mutants_selected_together() {
    let Some((_dir, outcomes)) = support::python_per_test() else {
        return;
    };
    // The covered mutants of `discount.py` select the large and the small test between them.
    let tests: BTreeSet<Option<u32>> = of(&outcomes, MutationPhase::Build)
        .iter()
        .map(|row| row.tests)
        .collect();
    assert_eq!(tests, BTreeSet::from([Some(2)]), "{:?}", outcomes.phases);
    let runs: BTreeSet<Option<u32>> = of(&outcomes, MutationPhase::Run)
        .iter()
        .map(|row| row.tests)
        .collect();
    assert_eq!(runs, BTreeSet::from([Some(1), Some(2)]));
}

#[test]
fn a_production_that_runs_no_mutant_records_no_phase() {
    let Some((dir, first)) = support::python() else {
        return;
    };
    let store =
        cupel_store::Store::open(&cupel_core::Writer::new(dir.path(), false).unwrap()).unwrap();
    let run: cupel_core::RunId = "run_01JA2B3C4D5E6F7G8H9JKMNP01".parse().unwrap();
    store
        .record_mutants(&run, &cupel_core::LangId::from("python"), &first)
        .unwrap();
    drop(store);
    let second = produce(dir.path(), "python", &Run::default()).unwrap();
    assert!(ran(&second).is_empty());
    assert!(second.phases.is_empty(), "{:?}", second.phases);
}

#[test]
fn a_schemata_file_records_its_rewrite_before_the_build_every_mutant_shares() {
    if !support::installed("cargo", "the Rust schemata breakdown") {
        return;
    }
    let dir = fixture("tests.mutation/rust");
    let outcomes = produce(dir.path(), "rust", &Run::default()).unwrap();
    let rewrites = of(&outcomes, MutationPhase::Rewrite);
    assert_eq!(rewrites.len(), 1, "{:?}", outcomes.phases);
    assert_eq!(
        (rewrites[0].file.as_deref(), rewrites[0].slot),
        (Some("src/lib.rs"), None)
    );
    let order: Vec<MutationPhase> = outcomes
        .phases
        .iter()
        .map(|row| row.phase)
        .filter(|phase| *phase != MutationPhase::Sync)
        .collect();
    let first_build = order
        .iter()
        .position(|phase| *phase == MutationPhase::Build)
        .unwrap();
    let rewrite = order
        .iter()
        .position(|phase| *phase == MutationPhase::Rewrite)
        .unwrap();
    assert!(rewrite < first_build, "{order:?}");
    assert!(
        !of(&outcomes, MutationPhase::Run).is_empty(),
        "{:?}",
        outcomes.phases
    );
}
