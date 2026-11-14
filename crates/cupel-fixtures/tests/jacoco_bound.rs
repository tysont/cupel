// ABOUTME: A Maven module that runs JaCoCo's agent itself (its parent binds prepare-agent, as jackson-core's does): `init`
// ABOUTME: writes `test` with the project's own `jacoco:report`, and the coverage run passes with one agent and maps Grade.java.
use std::fs;
use std::path::Path;

use cupel_fixtures::{copy_repo, run_cupel};
use serde_json::{Value, json};

/// The Java coverage fixture's sources under the jacoco-bound module's `pom.xml` and parent,
/// without its committed report or configuration, then `init --write`.
fn initialized() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("coverage/java", dir.path());
    fs::remove_dir_all(dir.path().join(".cupel")).unwrap();
    fs::remove_file(dir.path().join("cupel.toml")).unwrap();
    copy_repo("jacoco-bound/java", dir.path());
    let outcome = run_cupel(&["init", "--write", "--format", "json"], dir.path());
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    dir
}

/// The `[lang.java] coverage` command of the `cupel.toml` in `dir`.
fn coverage_command(dir: &Path) -> String {
    let text = fs::read_to_string(dir.join("cupel.toml")).unwrap();
    let config: toml::Value = toml::from_str(&text).unwrap();
    config["lang"]["java"]["coverage"]
        .as_str()
        .unwrap()
        .to_owned()
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

/// [`initialized`], when Maven is installed; `None`, said to be skipped, when not.
fn with_maven() -> Option<tempfile::TempDir> {
    if cupel_core::find_program("mvn").is_none() {
        eprintln!("SKIPPED: mvn not installed; the coverage run was not made");
        return None;
    }
    Some(initialized())
}

#[test]
fn init_runs_the_projects_own_report_goal_where_the_build_runs_the_agent() {
    let dir = initialized();
    assert_eq!(
        coverage_command(dir.path()),
        "mvn -B --no-transfer-progress test jacoco:report"
    );
}

/// The configured run passes under the project's one agent, and its report maps `Grade.java`:
/// `refund`, which no test calls, is the uncovered symbol, and `classify`'s failing grade the
/// uncovered branch, as with the goals by coordinate on the plain fixture. Needs Maven with the plugins in its
/// local repository (`make dev-setup`).
#[test]
fn the_coverage_run_passes_with_the_projects_agent_alone_and_maps_the_sources() {
    let Some(dir) = with_maven() else { return };
    let outcome = run_cupel(
        &["check", "--only", "tests.coverage", "--format", "json"],
        dir.path(),
    );
    let envelope: Value = serde_json::from_str(&outcome.stdout).unwrap();
    assert_eq!(
        (outcome.code, found(&envelope)),
        (
            Some(0),
            vec![
                (
                    "coverage.uncovered-branch".to_owned(),
                    "grade.Grade.classify".to_owned()
                ),
                (
                    "coverage.uncovered-symbol".to_owned(),
                    "grade.Grade.refund".to_owned()
                ),
            ]
        ),
        "{envelope}"
    );
    assert_eq!(envelope["checks"][0]["status"], json!("fail"));
}
