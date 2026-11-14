// ABOUTME: A Python project that chooses its coverage files itself (`[tool.coverage.run] include`) and makes warnings errors:
// ABOUTME: `init` writes a bare `--cov`, the coverage run passes, and the project's files are mapped and measured per test.
use std::fs;
use std::path::Path;

use cupel_fixtures::{copy_repo, run_cupel};
use serde_json::{Value, json};

/// A copy of the fixture with the `cupel.toml` that `init` writes for it.
fn initialized() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("coverage-config/python", dir.path());
    let outcome = run_cupel(&["init", "--write", "--format", "json"], dir.path());
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    dir
}

/// The `[lang.python] coverage` command of the `cupel.toml` in `dir`.
fn coverage_command(dir: &Path) -> String {
    let text = fs::read_to_string(dir.join("cupel.toml")).unwrap();
    let config: toml::Value = toml::from_str(&text).unwrap();
    config["lang"]["python"]["coverage"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// `cupel <args> --format json` in `dir`: its exit code and envelope.
fn run(dir: &Path, args: &[&str]) -> (Option<i32>, Value) {
    let mut all = args.to_vec();
    all.extend(["--format", "json"]);
    let outcome = run_cupel(&all, dir);
    let envelope = serde_json::from_str(&outcome.stdout)
        .unwrap_or_else(|_| panic!("not an envelope: {}", outcome.stdout));
    (outcome.code, envelope)
}

/// Each finding as `(kind, symbol)`.
fn found(envelope: &Value) -> Vec<(String, String)> {
    envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            (
                finding["kind"].as_str().unwrap_or_default().to_owned(),
                finding["symbol"]["display"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            )
        })
        .collect()
}

/// [`initialized`], when pytest and coverage are installed; `None`, said to be skipped, when
/// not.
fn with_tools() -> Option<tempfile::TempDir> {
    let missing = ["pytest", "coverage"]
        .into_iter()
        .find(|program| cupel_core::find_program(program).is_none());
    if let Some(program) = missing {
        eprintln!("SKIPPED: {program} not installed; the coverage run was not made");
        return None;
    }
    Some(initialized())
}

/// A `--cov=<source>` makes coverage.py ignore the project's `include` with a warning that
/// `filterwarnings = ["error"]` turns into a failed run; a bare `--cov` lets the project's
/// configuration choose the files.
#[test]
fn init_passes_a_bare_cov_when_the_project_chooses_its_coverage_files() {
    let dir = initialized();
    assert_eq!(
        coverage_command(dir.path()),
        "python -m pytest --cov --cov-report=lcov:{out}"
    );
}

/// The coverage run `init` configured passes: the included file's untested function is
/// uncovered, and the script outside the project's `include`, which coverage.py never
/// measures, is not reported as a file no test loaded.
#[test]
fn the_projects_coverage_configuration_chooses_the_files_and_the_run_passes() {
    let Some(dir) = with_tools() else { return };
    let (code, envelope) = run(dir.path(), &["check", "--only", "tests.coverage"]);
    assert_eq!(
        (code, found(&envelope)),
        (
            Some(0),
            vec![(
                "coverage.uncovered-symbol".to_owned(),
                "shop.pricing.surcharge".to_owned()
            )]
        ),
        "{envelope}"
    );
    assert_eq!(envelope["checks"][0]["status"], json!("fail"));
}

/// The joint run (one pytest run with test contexts, ADR-084) passes under the project's
/// configuration too, and its contexts name the test that ran the discount: a change to
/// `shop/pricing.py` then impacts exactly that test, from the map the joint run left.
#[test]
fn the_joint_runs_per_test_contexts_still_name_the_tests_of_each_file() {
    let Some(dir) = with_tools() else { return };
    let (code, envelope) = run(dir.path(), &["check", "--only", "tests.mutation"]);
    assert_eq!(
        (code, envelope["run"]["suite"]["python"].clone()),
        (
            Some(0),
            json!({"joint": "tool.pytest-cov-contexts", "mode": "full", "rerun": 1, "reused": 0,
                   "tagged": 0})
        ),
        "{envelope}"
    );
    let (code, impacted) = run(
        dir.path(),
        &["tests", "--impacted", "--paths", "shop/pricing.py"],
    );
    let tests: Vec<Value> = impacted["tests"]["impacted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|test| test["id"].clone())
        .collect();
    assert_eq!(
        (code, tests, impacted["tests"]["languages"][0]["reused"].clone()),
        (
            Some(0),
            vec![json!(
                "tests/test_pricing.py::test_a_tenth_off_a_hundred_is_ninety"
            )],
            json!(1)
        ),
        "{impacted}"
    );
}
