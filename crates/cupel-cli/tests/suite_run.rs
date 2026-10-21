// ABOUTME: One suite run through the binary on the Python per-test fixture (ADR-084): a dry run plans one joint run
// ABOUTME: where it would plan three, an unchanged tree reuses the cached baseline, and a red run is never cached.
mod support;

use std::time::SystemTime;

use serde_json::Value;
use support::Repo;

/// The toolchain table's pytest-cov command, which the joint run reproduces.
const DEFAULT: &str = "pytest --cov --cov-report=lcov:{out}";

/// A copy of the Python per-test fixture configuring `coverage`; `None` (reported as skipped)
/// without pytest or coverage.
fn fixture(coverage: &str) -> Option<Repo> {
    if cupel_core::find_program("coverage").is_none() {
        eprintln!("SKIPPED: coverage not installed; the joint run was not run");
        return None;
    }
    let repo = Repo::python_fixture("per-test")?;
    repo.write(
        "cupel.toml",
        &format!("[lang.python]\ncoverage = \"{coverage}\"\n"),
    );
    Some(repo)
}

/// `check --only tests.mutation` with `args`: its exit code and envelope.
fn check(repo: &Repo, args: &[&str]) -> (i32, Value) {
    let mut all = vec!["check", "--only", "tests.mutation"];
    all.extend(args);
    let outcome = repo.run(&all);
    (outcome.code, outcome.envelope())
}

/// What a dry run of the mutation check plans to make.
fn planned(repo: &Repo) -> Vec<String> {
    let (code, envelope) = check(repo, &["--dry-run"]);
    assert_eq!(code, 0, "{envelope}");
    serde_json::from_value(envelope["plan"]["would_run"][0]["needs"].clone()).unwrap()
}

#[test]
fn a_dry_run_plans_one_suite_run_where_separate_producers_would_run_three() {
    let (Some(joint), Some(apart)) = (
        fixture(DEFAULT),
        fixture("pytest --cov --cov-branch --cov-report=lcov:{out}"),
    ) else {
        return;
    };
    assert_eq!(
        (planned(&joint), planned(&apart)),
        (
            vec![
                "symbols".to_owned(),
                "mutation-outcomes".to_owned(),
                "test-conventions".to_owned(),
                "test-results+coverage-map+per-test-map(one run)".to_owned(),
            ],
            vec![
                "symbols".to_owned(),
                "mutation-outcomes".to_owned(),
                "test-conventions".to_owned(),
                "test-results(run)".to_owned(),
                "coverage-map(run)".to_owned(),
                "per-test-map(deep)".to_owned(),
            ]
        )
    );
}

/// When the joint run's cache entry was last written.
fn written(repo: &Repo) -> Option<SystemTime> {
    let dir = repo.path(".cupel/cache/suite/tool.pytest-cov-contexts");
    let entry = std::fs::read_dir(dir).ok()?.flatten().next()?.path();
    std::fs::metadata(entry.join("report"))
        .and_then(|meta| meta.modified())
        .ok()
}

#[test]
fn an_unchanged_tree_reuses_the_baseline_and_a_source_change_runs_the_suite_again() {
    let Some(repo) = fixture(DEFAULT) else { return };
    let (first, _) = check(&repo, &[]);
    let made = written(&repo);
    let (second, envelope) = check(&repo, &[]);
    let reused = written(&repo);
    let source = std::fs::read_to_string(repo.path("calc.py")).unwrap();
    repo.write("calc.py", &source.replace("a + b", "b + a"));
    let (third, _) = check(&repo, &[]);
    assert_eq!((first, second, third), (0, 0, 0), "{envelope}");
    assert!(made.is_some());
    assert_eq!(reused, made, "an unchanged tree runs no suite");
    assert_ne!(written(&repo), made, "a source change runs the suite again");
}

#[test]
fn a_red_baseline_is_not_cached_and_refuses_every_run_with_exit_4() {
    let Some(repo) = fixture(DEFAULT) else { return };
    repo.write(
        "tests/test_red.py",
        "def test_reports_a_wrong_sum():\n    assert 2 + 2 == 5\n",
    );
    let (first, _) = check(&repo, &[]);
    let made = written(&repo);
    let (second, envelope) = check(&repo, &[]);
    let kinds: Vec<&str> = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        (first, second, kinds),
        (4, 4, vec!["baseline.test-failed"]),
        "{envelope}"
    );
    assert_eq!(made, None, "a red baseline is not cached");
    assert_eq!(written(&repo), None, "nor when it is run again");
}

/// A suite that failed for a reason outside the tree (a file it reads elsewhere, as a full disk
/// or a missing service would) passes once the reason is gone, though the tree is unchanged.
#[test]
fn a_suite_that_failed_once_runs_again_and_passes_on_an_unchanged_tree() {
    let Some(repo) = fixture(DEFAULT) else { return };
    let outside = tempfile::tempdir().unwrap();
    let flag = outside.path().join("disk-full");
    std::fs::write(&flag, "").unwrap();
    repo.write(
        "tests/test_environment.py",
        &format!(
            "import os\n\n\ndef test_the_environment_is_healthy():\n    assert not os.path.exists({:?})\n",
            flag.display().to_string()
        ),
    );
    let (failed, _) = check(&repo, &[]);
    std::fs::remove_file(&flag).unwrap();
    let (passed, envelope) = check(&repo, &[]);
    assert_eq!((failed, passed), (4, 0), "{envelope}");
    assert!(written(&repo).is_some(), "the green run is cached");
}
