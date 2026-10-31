// ABOUTME: `engine.mutation` on real fixtures (ADR-080): the exact outcomes of the Python fixture's mutants, coverage
// ABOUTME: gating, and an engine given no baseline running nothing; skipped where pytest is not installed.
mod support;

use cupel_core::{
    Cancellation, Ctx, FactKind, MutantOutcome, MutationOutcomes, ProduceCtx, Scope, Writer,
};
use support::{registries, summary};

#[test]
fn the_engine_needs_sites_symbols_coverage_and_the_baseline() {
    let engine = registries()
        .producers
        .producers_for(FactKind::MutationOutcomes, &support::lang("python"))
        .find(|producer| producer.id().as_str() == cupel_mutate::ID)
        .unwrap();
    assert_eq!(
        engine.needs(),
        [
            FactKind::MutationSites,
            FactKind::Symbols,
            FactKind::CoverageMap,
            FactKind::TestResults
        ]
    );
    assert!(engine.wants().contains(&FactKind::PerTestMap));
    assert!(engine.wants().contains(&FactKind::ModuleGraph));
}

#[test]
fn with_no_baseline_the_engine_runs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ProduceCtx::new(
        Ctx::new(dir.path().to_path_buf()),
        Writer::new(dir.path(), false).unwrap().confined(),
        Cancellation::new(),
    );
    let engine = registries()
        .producers
        .producers_for(FactKind::MutationOutcomes, &support::lang("go"))
        .next()
        .unwrap();
    let outcomes = engine
        .produce(&ctx, &Scope::default(), &mut Vec::new())
        .unwrap()
        .downcast::<MutationOutcomes>()
        .unwrap();
    assert_eq!((outcomes.mutants.len(), outcomes.runner), (0, None));
}

#[test]
fn the_python_fixture_kills_all_but_its_weak_spots() {
    let Some((_dir, outcomes)) = support::python() else {
        return;
    };
    assert_eq!(
        summary(&outcomes),
        [
            "6 >=→> survived",
            "6 and→or killed",
            "7 -→+ killed",
            "8 *→/ survived (equivalent?)",
            "8 1→0 killed",
            "13 -→+ uncovered",
        ]
    );
    assert_eq!(
        outcomes.runner.as_ref().map(ToString::to_string),
        Some("tool.pytest".to_owned())
    );
}

#[test]
fn an_uncovered_mutant_is_never_run() {
    let Some((_dir, outcomes)) = support::python() else {
        return;
    };
    let uncovered: Vec<(usize, Option<u64>)> = outcomes
        .mutants
        .iter()
        .filter(|mutant| mutant.outcome == MutantOutcome::Uncovered)
        .map(|mutant| (mutant.tests.len(), mutant.duration_ms))
        .collect();
    assert_eq!(uncovered, [(0, None)]);
}

/// A file's mutants that end in an error leave every slot holding the original file, not the
/// reference's copy or a mutant (ADR-141 §3): here the reference run is red, since a test fails
/// wherever it runs from a mutation slot, as a cancelled or budget-stopped run would leave it.
#[test]
fn mutants_that_end_in_an_error_leave_the_slots_holding_the_original_file() {
    if !support::installed("pytest", "the Python mutation fixture") {
        return;
    }
    let dir = support::fixture("tests.mutation/python");
    std::fs::write(
        dir.path().join("tests/test_place.py"),
        "def test_runs_outside_a_mutation_slot():\n    assert \"cupel-mutate\" not in __file__\n",
    )
    .unwrap();
    let produced = support::produce(dir.path(), "python", &support::Run::default());
    assert!(produced.is_err(), "the reference run is red in a slot");
    let original = std::fs::read_to_string(dir.path().join("discount.py")).unwrap();
    let held: Vec<String> = (0..8)
        .map(|index| cupel_mutate::slots::slot_dir(dir.path(), "python", index))
        .filter_map(|slot| std::fs::read_to_string(slot.join("discount.py")).ok())
        .collect();
    let used = held.len().max(1);
    assert_eq!(
        held,
        vec![original; used],
        "every slot used holds the original"
    );
}

/// The unmutated file's tests over their share of `max_mutant_memory` end the production with an
/// error naming the setting: every mutant of the file would be over it too (ADR-148 §4).
#[test]
fn unmutated_tests_over_their_memory_share_are_an_error_naming_the_setting() {
    if !support::installed("pytest", "the Python mutation fixture") {
        return;
    }
    let dir = support::fixture("tests.mutation/python");
    let configs = tempfile::tempdir().unwrap();
    let toml = std::fs::read_to_string(dir.path().join("cupel.toml")).unwrap();
    let config = configs.path().join("tight.toml");
    std::fs::write(
        &config,
        format!("{toml}\n[checks.\"tests.mutation\"]\nmax_mutant_memory = \"1m\"\n"),
    )
    .unwrap();
    let run = support::Run {
        config: Some(config),
        ..support::Run::default()
    };
    let Err(cupel_core::Error::Process { program, message }) =
        support::produce(dir.path(), "python", &run)
    else {
        panic!("the production fails over the budget");
    };
    assert_eq!(program, "tool.pytest");
    // The share is the budget over the slots, which follow the machine's cores: it is named,
    // never as zero, beside the setting that sets it.
    let share = message
        .strip_prefix("the tests of the unmutated file held more than ")
        .and_then(|rest| rest.split_once(" of memory, one slot's share of `max_mutant_memory`"))
        .map(|(share, _)| share);
    assert!(
        share.is_some_and(|share| share.ends_with("KiB") || share == "1 MiB"),
        "{message}"
    );
}

/// A fixture copy's mutation slots, made under the machine's temporary directory by the production,
/// are removed with the copy, so no test leaves a slot directory behind.
#[test]
fn a_fixture_copys_mutation_slots_are_removed_with_it() {
    let Some((dir, _)) = support::python() else {
        return;
    };
    let slots = cupel_mutate::slots::slot_dir(dir.path(), "python", 0)
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf();
    assert!(slots.is_dir(), "{} is made", slots.display());
    drop(dir);
    assert!(!slots.exists(), "{} is removed", slots.display());
}
