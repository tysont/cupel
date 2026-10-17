// ABOUTME: Integration tests for configuration through the real binary: discovery, layered precedence,
// ABOUTME: the `run.config` echo, and configuration errors reported as exit-2 envelopes.
mod support;

use std::path::Path;

use serde_json::{Value, json};
use support::{cupel, run_with};
use tempfile::TempDir;

/// A repository root containing a `cupel.toml` with `text`.
fn repo(text: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::write(dir.path().join("cupel.toml"), text).expect("write cupel.toml");
    dir
}

fn cwd(dir: &Path) -> String {
    dir.to_str().expect("UTF-8 path").to_owned()
}

#[test]
fn an_unknown_key_in_the_config_file_is_a_usage_error_naming_the_key() {
    let dir = repo("depth = \"quick\"\ncolour = \"never\"\n");
    let config = dir.path().join("cupel.toml");
    let outcome = run_with(
        &mut cupel(),
        ["describe", "--config", config.to_str().unwrap()],
    );
    assert_eq!(outcome.code, 2);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("error"));
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    assert_eq!(finding["action"]["kind"], json!("configure"));
    let title = finding["title"].as_str().unwrap();
    assert!(title.contains("`colour`"), "{title}");
    let fix = finding["action"]["summary"].as_str().unwrap();
    assert!(
        fix.contains("`required_version`"),
        "valid keys listed: {fix}"
    );
    assert!(
        finding["detail"].as_str().unwrap().contains("line 2"),
        "{finding:#}"
    );
}

#[test]
fn config_errors_render_in_the_requested_format() {
    let dir = repo("colour = \"never\"\n");
    let outcome = run_with(
        &mut cupel(),
        ["describe", "--format", "text", "--cwd", &cwd(dir.path())],
    );
    assert_eq!(outcome.code, 2);
    assert!(
        outcome.stdout.starts_with("cupel describe: error"),
        "{}",
        outcome.stdout
    );
}

#[test]
fn a_missing_config_path_is_a_usage_error_about_config() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = run_with(
        &mut cupel(),
        [
            "describe",
            "--cwd",
            &cwd(dir.path()),
            "--config",
            "nowhere.toml",
        ],
    );
    assert_eq!(outcome.code, 2);
    let finding = outcome.finding();
    assert_eq!(finding["evidence"]["argument"], json!("--config"));
    assert!(
        finding["title"].as_str().unwrap().contains("nowhere.toml"),
        "{finding:#}"
    );
}

#[test]
fn an_undefined_profile_is_a_usage_error_listing_the_defined_ones() {
    let dir = repo("[profile.ci]\ndepth = \"deep\"\n[profile.loop]\ndepth = \"quick\"\n");
    let outcome = run_with(
        &mut cupel(),
        [
            "describe",
            "--cwd",
            &cwd(dir.path()),
            "--profile",
            "nightly",
        ],
    );
    assert_eq!(outcome.code, 2);
    let finding = outcome.finding();
    assert_eq!(finding["evidence"]["argument"], json!("--profile"));
    assert_eq!(finding["evidence"]["value"], json!("nightly"));
    assert_eq!(finding["evidence"]["valid"], json!(["ci", "loop"]));
}

#[test]
fn an_invalid_setting_variable_is_a_usage_error_naming_the_variable() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = run_with(
        cupel().env("CUPEL_DEPTH", "bottomless"),
        ["describe", "--cwd", &cwd(dir.path())],
    );
    assert_eq!(outcome.code, 2);
    let finding = outcome.finding();
    assert_eq!(finding["evidence"]["argument"], json!("CUPEL_DEPTH"));
    assert_eq!(finding["evidence"]["value"], json!("bottomless"));
}

#[test]
fn cupel_format_env_outranks_the_profile_format() {
    let dir = repo("[profile.x]\nformat = \"json\"\n");
    let root = cwd(dir.path());
    let profile_only = run_with(&mut cupel(), ["describe", "--cwd", &root, "--profile", "x"]);
    assert_eq!(profile_only.envelope()["status"], json!("pass"));
    let env_wins = run_with(
        cupel().env("CUPEL_FORMAT", "text"),
        ["describe", "--cwd", &root, "--profile", "x"],
    );
    assert!(
        env_wins.stdout.starts_with("cupel describe: pass"),
        "{}",
        env_wins.stdout
    );
    let flag_wins = run_with(
        cupel().env("CUPEL_FORMAT", "text"),
        [
            "describe",
            "--cwd",
            &root,
            "--profile",
            "x",
            "--format",
            "json",
        ],
    );
    assert_eq!(flag_wins.envelope()["status"], json!("pass"));
}

#[test]
fn the_config_file_format_applies_when_nothing_else_sets_one() {
    let dir = repo("format = \"text\"\n");
    let outcome = run_with(&mut cupel(), ["describe", "--cwd", &cwd(dir.path())]);
    assert_eq!(
        outcome.stdout.lines().next(),
        Some("cupel describe: pass (exit 0)")
    );
}

#[test]
fn run_config_echoes_the_effective_settings_and_their_sources() {
    let dir = repo("depth = \"quick\"\n[profile.ci]\nbudget = \"20s\"\n");
    let outcome = run_with(
        cupel().env("CUPEL_PROFILE", "ci"),
        ["describe", "--cwd", &cwd(dir.path()), "--format", "json"],
    );
    assert_eq!(outcome.code, 0);
    let envelope = outcome.envelope();
    let config = &envelope["run"]["config"];
    assert_eq!(
        config["sources"],
        json!(["flags", "profile", "cupel.toml"]),
        "{config:#}"
    );
    assert_eq!(config["effective"]["depth"], json!("quick"));
    assert_eq!(config["effective"]["format"], json!("json"));
    assert_eq!(config["effective"]["budget"], json!("20s"));
    assert_eq!(config["effective"]["profile"], json!("ci"));
    assert_eq!(envelope["run"]["depth"], json!("quick"));
    assert_eq!(envelope["run"]["budget_ms"], json!(20_000));
}

#[test]
fn without_a_config_file_everything_falls_through_to_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = run_with(&mut cupel(), ["describe", "--cwd", &cwd(dir.path())]);
    let envelope = outcome.envelope();
    assert_eq!(envelope["run"]["config"]["sources"], json!(["defaults"]));
    assert_eq!(
        envelope["run"]["config"]["effective"]["format"],
        Value::Null
    );
    assert_eq!(envelope["run"]["depth"], json!("standard"));
    assert_eq!(envelope["run"]["budget_ms"], Value::Null);
}

#[test]
fn cupel_format_env_is_reported_as_the_env_layer() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = run_with(
        cupel().env("CUPEL_FORMAT", "json"),
        ["describe", "--cwd", &cwd(dir.path())],
    );
    assert_eq!(
        outcome.envelope()["run"]["config"]["sources"],
        json!(["env", "defaults"])
    );
}

#[test]
fn depth_and_budget_flags_are_the_flags_layer() {
    let dir = repo("depth = \"quick\"\n");
    let outcome = run_with(
        &mut cupel(),
        [
            "check",
            "--cwd",
            &cwd(dir.path()),
            "--depth",
            "deep",
            "--budget",
            "5m",
        ],
    );
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["run"]["depth"], json!("deep"));
    assert_eq!(envelope["run"]["budget_ms"], json!(300_000));
    assert_eq!(
        envelope["run"]["config"]["sources"],
        json!(["flags", "cupel.toml", "defaults"])
    );
}

#[test]
fn an_invalid_budget_flag_is_a_usage_error_about_budget() {
    let outcome = run_with(&mut cupel(), ["check", "--budget", "soon"]);
    assert_eq!(outcome.code, 2);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    assert_eq!(finding["evidence"]["argument"], json!("--budget"));
}

#[test]
fn an_unregistered_language_in_the_config_is_a_usage_error_listing_the_languages() {
    let dir = repo("[lang.cobol]\ntest = \"make test\"\n");
    let outcome = run_with(&mut cupel(), ["describe", "--cwd", &cwd(dir.path())]);
    assert_eq!(outcome.code, 2, "{}", outcome.stdout);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    let text = finding.to_string();
    assert!(text.contains("cobol"), "{finding:#}");
    assert!(
        text.contains("typescript"),
        "registered languages listed: {finding:#}"
    );
}

#[test]
fn doctor_reports_an_unregistered_language_and_init_force_replaces_it() {
    let dir = repo("[roots.svc]\npath = \"svc\"\nlang = \"cobol\"\n");
    let doctor = run_with(&mut cupel(), ["doctor", "--cwd", &cwd(dir.path())]);
    let envelope = doctor.envelope();
    let kinds: Vec<&str> = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|finding| finding["kind"].as_str())
        .collect();
    assert!(kinds.contains(&"doctor.config-invalid"), "{kinds:?}");
    let init = run_with(
        &mut cupel(),
        ["init", "--force", "--write", "--cwd", &cwd(dir.path())],
    );
    assert_eq!(init.code, 0, "{}", init.stdout);
}
