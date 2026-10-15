// ABOUTME: The mutation performance stack on the Python fixture (ADR-080): per-test selection, reference-run timeouts,
// ABOUTME: store-based reuse (nothing reruns unchanged, a changed test reruns only what it covers), and depth.
mod support;

use std::fs;
use std::path::Path;

use cupel_core::{ChangedLines, CostClass, LangId, MutantOutcome, MutationOutcomes, RunId, Writer};
use cupel_store::Store;
use support::{Run, fixture, produce, with_outcome};

/// Whether the Python tools are here; prints the skip line when they are not.
fn python_tools() -> bool {
    support::installed("pytest", "the Python mutation fixture")
}

/// Indexes `outcomes` in the repository's store as run `n` would.
fn record(root: &Path, n: u8, outcomes: &MutationOutcomes) {
    let store = Store::open(&Writer::new(root, false).unwrap()).unwrap();
    let run: RunId = format!("run_01JA2B3C4D5E6F7G8H9JKMNP{n:02}")
        .parse()
        .unwrap();
    store
        .record_mutants(&run, &LangId::from("python"), outcomes)
        .unwrap();
}

/// The mutants that ran this time (not reused, not uncovered), as `line from→to`.
fn executed(outcomes: &MutationOutcomes) -> Vec<String> {
    outcomes
        .mutants
        .iter()
        .filter(|mutant| !mutant.reused && mutant.outcome != MutantOutcome::Uncovered)
        .map(|mutant| format!("{} {}→{}", mutant.span.line, mutant.from, mutant.to))
        .collect()
}

#[test]
fn a_second_run_with_no_changes_runs_no_mutant() {
    if !python_tools() {
        return;
    }
    let dir = fixture("tests.mutation/python");
    let first = produce(dir.path(), "python", &Run::default()).unwrap();
    record(dir.path(), 1, &first);
    let second = produce(dir.path(), "python", &Run::default()).unwrap();
    assert_eq!(executed(&second), Vec::<String>::new());
    assert_eq!(
        with_outcome(&second, MutantOutcome::Survived),
        with_outcome(&first, MutantOutcome::Survived)
    );
}

#[test]
fn a_changed_test_reruns_only_the_mutants_it_covers() {
    if !python_tools() {
        return;
    }
    let dir = fixture("tests.mutation/python");
    let first = produce(dir.path(), "python", &Run::default()).unwrap();
    if !first.per_test {
        eprintln!("SKIPPED: pytest-cov not installed; per-test reuse was not run");
        return;
    }
    record(dir.path(), 1, &first);
    // Only the small order's test changes: it runs lines 6 and 8, not line 7.
    let small = dir.path().join("tests/test_small.py");
    let text = fs::read_to_string(&small).unwrap();
    fs::write(&small, text.replace("(50, True) == 50", "(60, True) == 60")).unwrap();
    let second = produce(dir.path(), "python", &Run::default()).unwrap();
    assert_eq!(executed(&second), ["6 >=→>", "6 and→or", "8 *→/", "8 1→0"]);
}

#[test]
fn with_a_per_test_map_a_mutant_runs_only_the_tests_that_cover_it() {
    if !python_tools() {
        return;
    }
    let dir = fixture("tests.mutation/python");
    let outcomes = produce(dir.path(), "python", &Run::default()).unwrap();
    if !outcomes.per_test {
        eprintln!("SKIPPED: pytest-cov not installed; per-test selection was not run");
        return;
    }
    let tests: Vec<(u32, &str, Vec<&str>)> = outcomes
        .mutants
        .iter()
        .map(|m| {
            (
                m.span.line,
                m.to.as_str(),
                m.tests.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    let large = "tests/test_large.py::test_discounts_a_large_member_order";
    let small = "tests/test_small.py::test_keeps_a_small_order";
    assert_eq!(
        tests,
        [
            (6, ">", vec![large, small]),
            (6, "or", vec![large, small]),
            (7, "+", vec![large]),
            (8, "/", vec![small]),
            (8, "0", vec![small]),
            (13, "+", vec![]),
        ]
    );
}

#[test]
fn a_mutant_that_never_finishes_times_out() {
    if !python_tools() {
        return;
    }
    let dir = fixture("tests.mutation/python");
    fs::write(
        dir.path().join("count.py"),
        "def count(n):\n    i = 0\n    while i < n:\n        i += 1\n    return i\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("tests/test_count.py"),
        "from count import count\n\n\ndef test_counts():\n    assert count(3) == 3\n",
    )
    .unwrap();
    let toml = fs::read_to_string(dir.path().join("cupel.toml")).unwrap();
    fs::write(
        dir.path().join("cupel.toml"),
        format!("{toml}mutation = {{ timeout_factor = 1 }}\n"),
    )
    .unwrap();
    let outcomes = produce(dir.path(), "python", &Run::default()).unwrap();
    let counted: Vec<String> = outcomes
        .mutants
        .iter()
        .filter(|mutant| mutant.symbol.file == "count.py")
        .map(|mutant| {
            format!(
                "{} {}→{} {}",
                mutant.span.line, mutant.from, mutant.to, mutant.outcome
            )
        })
        .collect();
    assert_eq!(
        counted,
        [
            "2 0→1 survived",
            "3 <→<= killed",
            "4 +=→-= timeout",
            "4 1→0 timeout"
        ]
    );
}

#[test]
fn standard_depth_with_a_range_mutates_only_the_symbols_it_changed() {
    if !python_tools() {
        return;
    }
    let dir = fixture("tests.mutation/python");
    let changed: ChangedLines = [("discount.py".to_owned(), vec![(13, 13)])].into();
    let standard = Run {
        depth: Some(CostClass::Standard),
        changed: Some(changed.clone()),
    };
    let outcomes = produce(dir.path(), "python", &standard).unwrap();
    assert_eq!(support::summary(&outcomes), ["13 -→+ uncovered"]);
    let deep = Run {
        depth: Some(CostClass::Deep),
        changed: Some(changed),
    };
    let outcomes = produce(dir.path(), "python", &deep).unwrap();
    assert_eq!(outcomes.mutants.len(), 6);
    assert_eq!(outcomes.depth, CostClass::Deep);
}
