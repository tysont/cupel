// ABOUTME: A narrowed mutant's tests run one at a time, fastest first, each under its own cap (ADR-160), on the
// ABOUTME: ordering fixture: a hot path a fast and a slow test both kill, and a loop whose mutants never end.
mod support;

use cupel_core::{Mutant, MutantOutcome, MutationOutcomes};

// It sorts after the slow test, `tests/test_after_waiting.py`, by name, so only ordering by duration runs it first.
const FAST: &str = "tests/test_without_waiting.py::test_scales_quickly";
const COUNTDOWN: &str = "tests/test_countdown.py::test_counts_down_three_steps";

/// The ordering fixture's outcomes at `deep`, when its mutants ran by a per-test map; `None` (reported as skipped)
/// without pytest or pytest-cov.
fn ordering() -> Option<(support::Fixture, MutationOutcomes)> {
    if !support::installed("pytest", "the ordering fixture") {
        return None;
    }
    let dir = support::fixture("tests.mutation/python-ordering");
    let outcomes = support::produce(dir.path(), "python", &support::deep()).unwrap();
    if !outcomes.per_test {
        eprintln!("SKIPPED: pytest-cov not installed; per-test selection was not run");
        return None;
    }
    Some((dir, outcomes))
}

fn in_file<'a>(outcomes: &'a MutationOutcomes, file: &str) -> Vec<&'a Mutant> {
    outcomes
        .mutants
        .iter()
        .filter(|mutant| mutant.symbol.file == file)
        .collect()
}

/// Every mutant of the hot path is killed by the fast test alone, which runs first: the slow test, which would kill
/// it too after six seconds, never runs, and each mutant is decided in a fraction of that.
#[test]
fn the_fast_killing_test_runs_first_and_kills_a_hot_path_mutant_in_seconds() {
    let Some((_dir, outcomes)) = ordering() else {
        return;
    };
    let hot = in_file(&outcomes, "hot.py");
    assert!(!hot.is_empty(), "{:?}", support::summary(&outcomes));
    for mutant in hot {
        let line = format!("{} {}→{}", mutant.span.line, mutant.from, mutant.to);
        assert_eq!(
            (
                mutant.outcome,
                mutant.tests.as_slice(),
                mutant.killing_tests.as_slice(),
                mutant.hung.as_deref()
            ),
            (
                MutantOutcome::Killed,
                [FAST.to_owned()].as_slice(),
                [FAST.to_owned()].as_slice(),
                None
            ),
            "{line}"
        );
        let ms = mutant.duration_ms.unwrap();
        assert!(ms < 3_000, "{line} took {ms} ms");
    }
}

/// A mutant that makes the countdown's loop never end is killed when its fast test passes its own cap, named as the
/// test that hung; one that makes the loop end wrong fails the test.
#[test]
fn a_loop_that_never_ends_is_a_hang_that_kills_and_names_its_test() {
    let Some((_dir, outcomes)) = ordering() else {
        return;
    };
    let decided: Vec<(String, MutantOutcome, Option<&str>)> = in_file(&outcomes, "countdown.py")
        .into_iter()
        .map(|mutant| {
            (
                format!("{} {}→{}", mutant.span.line, mutant.from, mutant.to),
                mutant.outcome,
                mutant.hung.as_deref(),
            )
        })
        .collect();
    let hangs: Vec<&str> = decided
        .iter()
        .filter(|(_, _, hung)| hung.is_some())
        .map(|(line, _, _)| line.as_str())
        .collect();
    assert_eq!(hangs, ["8 -→+", "8 1→0"], "{decided:?}");
    for (line, outcome, hung) in &decided {
        assert_eq!(*outcome, MutantOutcome::Killed, "{line}");
        assert!(hung.is_none_or(|test| test == COUNTDOWN), "{line}");
    }
}
