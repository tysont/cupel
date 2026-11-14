// ABOUTME: A tools.deps Clojure project shaped like clj-yaml (a pom.xml template, Java sources tools.build compiles, a
// ABOUTME: :test alias whose :main-opts run its own runner): one Clojure root, and Cupel's coverage run passes.
use std::fs;

use cupel_fixtures::{copy_repo, run_cupel};
use serde_json::{Value, json};

/// A copy of the fixture with the `cupel.toml` that `init --write` writes for it.
fn initialized() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("tools-build", dir.path());
    let outcome = run_cupel(&["init", "--write", "--format", "json"], dir.path());
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    dir
}

/// The `cupel.toml` that `init --write` writes for a copy of the fixture.
fn initialized_config() -> toml::Value {
    let dir = initialized();
    toml::from_str(&fs::read_to_string(dir.path().join("cupel.toml")).unwrap()).unwrap()
}

/// The keys of the table at `key` in `config`, empty without one.
fn keys(config: &toml::Value, key: &str) -> Vec<String> {
    config
        .get(key)
        .and_then(toml::Value::as_table)
        .map(|table| table.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn init_writes_no_java_runner_for_a_pom_template_without_coordinates() {
    let config = initialized_config();
    assert_eq!(keys(&config, "lang"), ["clojure"]);
    assert_eq!(keys(&config, "roots"), Vec::<String>::new());
}

/// `cupel check --only tests.coverage` on [`initialized`]: its exit code, the check's status, and
/// each finding's kind; `None`, said to be skipped, when the Clojure CLI is not installed.
fn coverage_run() -> Option<(Option<i32>, Value, Vec<Value>)> {
    if cupel_core::find_program("clojure").is_none() {
        eprintln!("SKIPPED: clojure not installed; the coverage run was not made");
        return None;
    }
    let dir = initialized();
    let outcome = run_cupel(
        &["check", "--only", "tests.coverage", "--format", "json"],
        dir.path(),
    );
    let envelope: Value = serde_json::from_str(&outcome.stdout)
        .unwrap_or_else(|_| panic!("not an envelope: {}", outcome.stdout));
    let kinds = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["kind"].clone())
        .collect();
    Some((outcome.code, envelope["checks"][0]["status"].clone(), kinds))
}

/// Cloverage runs with the `:test` alias's paths but not its `:main-opts`, so its own `-m` main
/// runs the tests and the one function, tested, is covered.
#[test]
fn the_coverage_run_passes_where_the_test_alias_runs_another_runner() {
    let Some(run) = coverage_run() else { return };
    assert_eq!(run, (Some(0), json!("pass"), Vec::<Value>::new()));
}
