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
