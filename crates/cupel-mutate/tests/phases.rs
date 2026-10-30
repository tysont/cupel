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

/// A `--config` file in `dir`: the Rust fixture's `cupel.toml` at `root` with two mutation
/// slots, and `runner` as its runner when given.
fn two_slots(
    root: &std::path::Path,
    dir: &std::path::Path,
    runner: Option<&str>,
) -> std::path::PathBuf {
    let toml = std::fs::read_to_string(root.join("cupel.toml")).unwrap();
    let runner = runner.map_or_else(String::new, |runner| format!("runner = \"{runner}\"\n"));
    let path = dir.join("two-slots.toml");
    std::fs::write(&path, format!("{toml}{runner}mutation = {{ jobs = 2 }}\n")).unwrap();
    path
}

/// The Rust fixture's outcomes in a fresh copy, with two slots and `runner`.
fn rust_outcomes(runner: Option<&str>) -> (tempfile::TempDir, MutationOutcomes) {
    rust_outcomes_after(runner, |_| {})
}

/// [`rust_outcomes`] after `change` to the copy.
fn rust_outcomes_after(
    runner: Option<&str>,
    change: fn(&std::path::Path),
) -> (tempfile::TempDir, MutationOutcomes) {
    let dir = fixture("tests.mutation/rust");
    change(dir.path());
    let configs = tempfile::tempdir().unwrap();
    let run = Run {
        config: Some(two_slots(dir.path(), configs.path(), runner)),
        ..Run::default()
    };
    let outcomes = produce(dir.path(), "rust", &run).unwrap();
    (dir, outcomes)
}

#[test]
fn a_runner_that_builds_once_serves_every_slot_from_one_build_in_the_first() {
    if !support::installed("cargo-nextest", "the Rust shared-build test") {
        return;
    }
    let (dir, outcomes) = rust_outcomes(Some("cargo-nextest"));
    let before_runs: Vec<(MutationPhase, Option<u32>)> = outcomes
        .phases
        .iter()
        .filter(|row| {
            !matches!(
                row.phase,
                MutationPhase::Plan | MutationPhase::Sync | MutationPhase::Run
            )
        })
        .map(|row| (row.phase, row.slot))
        .collect();
    assert_eq!(
        before_runs,
        [
            (MutationPhase::Rewrite, None),
            (MutationPhase::Build, Some(0)),
            (MutationPhase::Validate, Some(0)),
        ],
        "{:?}",
        outcomes.phases
    );
    assert_eq!(
        slots(&outcomes, MutationPhase::Run),
        BTreeSet::from([Some(0), Some(1)]),
        "the mutants ran in both slots: {:?}",
        outcomes.phases
    );
    let second = cupel_mutate::slots::slot_dir(dir.path(), "rust", 1);
    assert!(
        !second.join("target/debug").exists(),
        "the second slot built nothing"
    );
    let (_other, alone) = rust_outcomes(None);
    assert_eq!(
        support::summary(&outcomes),
        support::summary(&alone),
        "one shared build finds what a build in every slot finds"
    );
}

/// A function of the reader's name in the fixture's `src/lib.rs`, so no schemata build of it
/// compiles and every mutant of the file runs as a file copy.
fn taken_name(root: &std::path::Path) {
    let lib = root.join("src/lib.rs");
    let text = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(
        &lib,
        format!("{text}\n#[allow(dead_code)]\nfn cupel_mutant_id() -> u32 {{\n    7\n}}\n"),
    )
    .unwrap();
}

#[test]
fn under_a_runner_that_builds_once_file_copies_run_in_the_first_slot_alone() {
    if !support::installed("cargo-nextest", "the Rust shared-build file-copy test") {
        return;
    }
    let (dir, outcomes) = rust_outcomes_after(Some("cargo-nextest"), taken_name);
    let copies = outcomes
        .mutants
        .iter()
        .filter(|mutant| mutant.strategy == Some(cupel_core::MutationStrategy::FileCopy))
        .count();
    assert_eq!(copies, 5, "{:?}", outcomes.mutants);
    assert_eq!(
        slots(&outcomes, MutationPhase::Run),
        BTreeSet::from([Some(0)]),
        "every file copy ran where the build is: {:?}",
        outcomes.phases
    );
    let second = cupel_mutate::slots::slot_dir(dir.path(), "rust", 1);
    assert!(
        !second.join("target/debug").exists(),
        "the second slot built nothing"
    );
}

/// A greeting `String + &str`, whose `+` → `-` does not type-check under schemata, and a test
/// that covers it.
fn greeting(root: &std::path::Path) {
    let lib = root.join("src/lib.rs");
    let text = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(
        &lib,
        format!(
            "{text}\n/// A greeting.\npub fn greet(name: &str) -> String {{\n    String::from(\"hi \") + name\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("tests/greet.rs"),
        "#[test]\nfn greets() {\n    assert_eq!(discount::greet(\"al\"), \"hi al\");\n}\n",
    )
    .unwrap();
}

#[test]
fn under_a_runner_that_builds_once_a_gate_that_falls_back_runs_after_the_others_where_the_build_is()
{
    if !support::installed("cargo-nextest", "the Rust shared-build fallback test")
        || !support::installed("cargo-llvm-cov", "the Rust shared-build fallback test")
    {
        return;
    }
    let (dir, outcomes) = rust_outcomes_after(Some("cargo-nextest"), greeting);
    let built: Vec<(u32, String, Option<cupel_core::MutationStrategy>)> = outcomes
        .mutants
        .iter()
        .filter(|mutant| mutant.outcome != cupel_core::MutantOutcome::Uncovered)
        .map(|mutant| {
            (
                mutant.span.line,
                mutant.outcome.to_string(),
                mutant.strategy,
            )
        })
        .collect();
    let schemata = Some(cupel_core::MutationStrategy::Schemata);
    assert_eq!(
        built,
        [
            (5, "survived".to_owned(), schemata),
            (5, "killed".to_owned(), schemata),
            (6, "killed".to_owned(), schemata),
            (8, "survived".to_owned(), schemata),
            (8, "killed".to_owned(), schemata),
            (
                24,
                "unviable".to_owned(),
                Some(cupel_core::MutationStrategy::FileCopy)
            ),
        ]
    );
    let second = cupel_mutate::slots::slot_dir(dir.path(), "rust", 1);
    assert!(
        !second.join("target/debug").exists(),
        "the second slot built nothing"
    );
}
