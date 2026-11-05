// ABOUTME: A narrowed mutant's tests run one at a time, fastest first, each under its own cap (ADR-160), on the
// ABOUTME: ordering fixture: a hot path a fast and a slow test both kill, a loop whose mutants never end, and slowness.
mod support;

use std::fs;
use std::path::Path;

use cupel_core::{MutantOutcome, MutationOutcomes};

// It sorts after the slow test, `tests/test_after_waiting.py`, by name, so only ordering by duration runs it first.
const FAST: &str = "tests/test_without_waiting.py::test_scales_quickly";
const SLOW: &str = "tests/test_after_waiting.py::test_scales_slowly";
const COUNTDOWN: &str = "tests/test_countdown.py::test_counts_down_three_steps";

/// The outcomes at `deep` of a copy of the ordering fixture that `change` has changed, when its mutants ran by a
/// per-test map; `None` (reported as skipped) without pytest or pytest-cov.
fn ordering(change: impl Fn(&Path)) -> Option<MutationOutcomes> {
    if !support::installed("pytest", "the ordering fixture") {
        return None;
    }
    let dir = support::fixture("tests.mutation/python-ordering");
    change(dir.path());
    let outcomes = support::produce(dir.path(), "python", &support::deep()).unwrap();
    if !outcomes.per_test {
        eprintln!("SKIPPED: pytest-cov not installed; per-test selection was not run");
        return None;
    }
    Some(outcomes)
}

/// Removes `files` from the copy at `root`.
fn remove(root: &Path, files: &[&str]) {
    for file in files {
        fs::remove_file(root.join(file)).unwrap();
    }
}

/// Each mutant of `file` in `outcomes` as `line from→to` beside what `of` reads from it.
fn of<T>(
    outcomes: &MutationOutcomes,
    file: &str,
    of: impl Fn(&cupel_core::Mutant) -> T,
) -> Vec<(String, T)> {
    outcomes
        .mutants
        .iter()
        .filter(|mutant| mutant.symbol.file == file)
        .map(|mutant| {
            let line = format!("{} {}→{}", mutant.span.line, mutant.from, mutant.to);
            (line, of(mutant))
        })
        .collect()
}

/// The hot path's one mutant.
const SCALED: &str = "6 *→/";

/// A row of `of` for the hot path's one mutant.
fn scaled<T>(value: T) -> Vec<(String, T)> {
    vec![(SCALED.to_owned(), value)]
}

/// Every mutant of the hot path is killed by the fast test alone, which runs first: the slow test, which would kill
/// it too after six seconds, never runs, and each mutant is decided in a fraction of that.
#[test]
fn the_fast_killing_test_runs_first_and_kills_a_hot_path_mutant_in_seconds() {
    let Some(outcomes) = ordering(|_| {}) else {
        return;
    };
    let fast = vec![FAST.to_owned()];
    assert_eq!(
        of(&outcomes, "hot.py", |mutant| mutant.outcome),
        scaled(MutantOutcome::Killed)
    );
    assert_eq!(
        of(&outcomes, "hot.py", |mutant| mutant.tests.clone()),
        scaled(fast.clone())
    );
    assert_eq!(
        of(&outcomes, "hot.py", |mutant| mutant.killing_tests.clone()),
        scaled(fast)
    );
    assert_eq!(
        of(&outcomes, "hot.py", |mutant| mutant.hung.clone()),
        scaled(None)
    );
    let quick = |mutant: &cupel_core::Mutant| mutant.duration_ms.is_some_and(|ms| ms < 3_000);
    assert_eq!(of(&outcomes, "hot.py", quick), scaled(true));
}

/// A mutant that makes the countdown's loop never end is killed when its fast test passes its own cap, named as the
/// test that hung; one that makes the loop end wrong fails the test.
#[test]
fn a_loop_that_never_ends_is_a_hang_that_kills_and_names_its_test() {
    let Some(outcomes) = ordering(|_| {}) else {
        return;
    };
    let killed = MutantOutcome::Killed;
    let lines = [
        "6 0→1", "7 >→>=", "7 0→1", "8 -→+", "8 1→0", "9 +→-", "9 1→0",
    ];
    assert_eq!(
        of(&outcomes, "countdown.py", |mutant| mutant.outcome),
        lines.map(|line| (line.to_owned(), killed))
    );
    let (none, countdown) = (None, Some(COUNTDOWN.to_owned()));
    let hung = [
        none.clone(),
        none.clone(),
        none.clone(),
        countdown.clone(),
        countdown,
        none.clone(),
        none,
    ];
    assert_eq!(
        of(&outcomes, "countdown.py", |mutant| mutant.hung.clone()),
        lines
            .map(str::to_owned)
            .into_iter()
            .zip(hung)
            .collect::<Vec<_>>()
    );
}

/// The ordering fixture without its fast tests and with three seconds for a mutant's whole run.
fn capped(root: &Path) {
    remove(
        root,
        &[
            "tests/test_without_waiting.py",
            "tests/test_countdown.py",
            "countdown.py",
        ],
    );
    let toml = fs::read_to_string(root.join("cupel.toml")).unwrap();
    let capped = format!("{toml}\n[checks.\"tests.mutation\"]\nmax_mutant_timeout = \"3s\"\n");
    fs::write(root.join("cupel.toml"), capped).unwrap();
}

/// With only the slow test covering the hot path and three seconds for a mutant's whole run, the slow test cannot
/// finish: each mutant is a timeout, not killed and not scored, naming the slow test and its baseline duration.
#[test]
fn a_mutant_whose_tests_cannot_finish_within_the_cap_is_a_timeout_naming_the_slowest() {
    let Some(outcomes) = ordering(capped) else {
        return;
    };
    assert_eq!(
        of(&outcomes, "hot.py", |mutant| mutant.outcome),
        scaled(MutantOutcome::Timeout)
    );
    let slowest = |mutant: &cupel_core::Mutant| -> Vec<String> {
        mutant.slowest.iter().map(|test| test.id.clone()).collect()
    };
    assert_eq!(
        of(&outcomes, "hot.py", slowest),
        scaled(vec![SLOW.to_owned()])
    );
    let timed = |mutant: &cupel_core::Mutant| mutant.slowest[0].baseline_ms >= Some(6_000);
    assert_eq!(of(&outcomes, "hot.py", timed), scaled(true));
}
