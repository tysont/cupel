// ABOUTME: A Leiningen project end to end: `project.clj` without `deps.edn` is a Clojure root, `init` writes `lein test`
// ABOUTME: and Cloverage by coordinate, and its tests, coverage, and mutants run through Leiningen (G2, ADR-234, ADR-239).
use cupel_core::{Status, find_program};
use cupel_fixtures::{copy_repo, run_cupel};
use serde_json::{Value, json};

#[test]
fn init_makes_a_leiningen_project_a_clojure_root_tested_with_lein_test() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("leiningen/clojure", dir.path());

    let outcome = run_cupel(&["init", "--dry-run", "--format", "json"], dir.path());

    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    assert_eq!(outcome.envelope().status, Status::Plan);
    outcome.assert_golden("init/leiningen");
}

/// The checks that need test results, each with its status and its headline metric (line
/// coverage, flaky tests, mutants killed), after `init --write` and `check` in a copy of `repo`.
fn tested(repo: &str) -> Vec<(Value, Value, Value)> {
    let dir = tempfile::tempdir().unwrap();
    copy_repo(repo, dir.path());
    let init = run_cupel(&["init", "--write", "--format", "json"], dir.path());
    assert_eq!(init.code, Some(0), "{}", init.stdout);
    let outcome = run_cupel(
        &[
            "check",
            "--only",
            "tests.coverage",
            "--only",
            "tests.flaky",
            "--only",
            "tests.mutation",
            "--format",
            "json",
        ],
        dir.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let envelope: Value = serde_json::from_str(&outcome.stdout).unwrap();
    envelope["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|check| {
            let metric = match check["id"].as_str().unwrap() {
                "tests.coverage" => "line",
                "tests.flaky" => "flaky",
                _ => "killed",
            };
            (
                check["id"].clone(),
                check["status"].clone(),
                check["metrics"][metric].clone(),
            )
        })
        .collect()
}

/// Every check passes, the one function covered, nothing flaky, and its one mutant killed.
fn all_pass() -> Vec<(Value, Value, Value)> {
    vec![
        (json!("tests.coverage"), json!("pass"), json!(1.0)),
        (json!("tests.flaky"), json!("pass"), json!(0.0)),
        (json!("tests.mutation"), json!("pass"), json!(1.0)),
    ]
}

/// Whether `lein` is installed; a test that needs it says it was skipped when not.
fn lein() -> bool {
    let found = find_program("lein").is_some();
    if !found {
        eprintln!("SKIPPED: lein not installed; the Leiningen fixture was not run");
    }
    found
}

/// What `init --write` wrote is what `check` runs: the tests, Cloverage, and the mutants all run
/// through Leiningen, with no `deps.edn` (ADR-239). Skipped without `lein`.
#[test]
fn a_leiningen_root_s_tests_coverage_and_mutants_run_through_leiningen() {
    if lein() {
        assert_eq!(tested("leiningen/clojure"), all_pass());
    }
}

/// A lein-sub aggregator, which depends on its subproject's unpublished jar as ring's does, is
/// not run; its subproject, a root of its own, is (ADR-239). Skipped without `lein`.
#[test]
fn a_leiningen_aggregator_s_subprojects_are_tested_and_the_aggregator_is_not_run() {
    if lein() {
        assert_eq!(tested("leiningen-sub/clojure"), all_pass());
    }
}
