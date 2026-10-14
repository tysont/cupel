// ABOUTME: Integration tests for `cupel suppress` and suppressions in runs: the committable file, the
// ABOUTME: store mirror, suppressed findings as info, expiry by date and ref, and the exit-3 rule (ADR-047).
mod support;

use std::fs;
use std::process::Command as Process;

use cupel_core::{CheckId, FindingId};
use serde_json::{Value, json};
use support::{Outcome, Repo, cupel};

const MISSING_RUNNER: &str = "[lang.rust]\ntest = \"cupel-test-no-such-runner --all\"\n";

fn runner_id() -> String {
    FindingId::for_subject(
        &CheckId::from("doctor"),
        "doctor.tool-missing",
        "cupel-test-no-such-runner",
    )
    .to_string()
}

/// A repository whose `doctor` reports the missing runner, after one `doctor` run.
fn doctored() -> Repo {
    let repo = Repo::new();
    repo.write("cupel.toml", MISSING_RUNNER);
    assert_eq!(repo.run(&["doctor"]).code, 0);
    repo
}

fn finding<'a>(envelope: &'a Value, id: &str) -> &'a Value {
    envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == json!(id))
        .unwrap_or_else(|| panic!("no {id} in {envelope}"))
}

fn kinds(envelope: &Value) -> Vec<String> {
    envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].as_str().unwrap().to_owned())
        .collect()
}

fn usage_argument(outcome: &Outcome) -> Value {
    assert_eq!(outcome.code, 2, "{}", outcome.stdout);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    finding["evidence"]["argument"].clone()
}

#[test]
fn suppressing_an_unknown_or_malformed_finding_is_a_usage_error() {
    let repo = Repo::new();
    for id in ["fnd_abcdef", "nonsense"] {
        let outcome = repo.run(&["suppress", id, "--reason", "x"]);
        assert_eq!(usage_argument(&outcome), json!("FINDING"), "{id}");
    }
    assert!(!repo.path(".cupel/suppressions.toml").exists());
}

#[test]
fn a_malformed_until_is_a_usage_error() {
    let repo = doctored();
    let outcome = repo.run(&[
        "suppress",
        &runner_id(),
        "--reason",
        "x",
        "--until",
        "2026-13-40",
    ]);
    assert_eq!(usage_argument(&outcome), json!("--until"));
}

#[test]
fn suppress_writes_the_file_mirrors_it_and_later_runs_show_the_finding_as_info() {
    let repo = doctored();
    let id = runner_id();

    let outcome = repo.run(&["suppress", &id, "--reason", "runner arrives in P3"]);

    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(finding(&envelope, &id)["suppressed"], json!(true));
    assert_eq!(finding(&envelope, &id)["severity"], json!("info"));
    let file = fs::read_to_string(repo.path(".cupel/suppressions.toml")).unwrap();
    assert!(file.contains(&format!("finding = \"{id}\"")), "{file}");
    assert!(file.contains("reason = \"runner arrives in P3\""), "{file}");
    let mirrored = repo.store().suppressions().unwrap();
    assert_eq!(mirrored.len(), 1);
    assert_eq!(mirrored[0].finding.to_string(), id);

    let doctor = repo.run(&["doctor"]).envelope();
    let suppressed = finding(&doctor, &id);
    assert_eq!(suppressed["suppressed"], json!(true));
    assert_eq!(suppressed["severity"], json!("info"));
    assert_eq!(doctor["summary"]["suppressed"], json!(1));
}

#[test]
fn the_suppressions_file_is_the_one_cupel_file_git_would_see() {
    let repo = doctored();
    repo.run(&["suppress", &runner_id(), "--reason", "x"]);
    let gitignore = fs::read_to_string(repo.path(".cupel/.gitignore")).unwrap();
    assert_eq!(gitignore, "*\n!suppressions.toml\n");
}

#[test]
fn a_dry_run_shows_the_file_diff_and_writes_nothing() {
    let repo = doctored();
    let before = repo.tree();
    let outcome = repo.run(&["suppress", &runner_id(), "--reason", "x", "--dry-run"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("plan"));
    let write = &envelope["plan"]["would_write"][0];
    assert_eq!(write["path"], json!(".cupel/suppressions.toml"));
    let finding_line = write["diff"]
        .as_str()
        .unwrap()
        .lines()
        .find(|line| line.starts_with("+finding"))
        .map(str::to_owned);
    assert_eq!(
        finding_line,
        Some(format!("+finding = \"{}\"", runner_id()))
    );
    assert_eq!(repo.tree(), before);
}

/// The runner finding's ID and the `cupel doctor` envelope after suppressing that finding with
/// `reason` until `until`.
fn doctor_after_suppressing(reason: &str, until: &str) -> (String, Value) {
    let repo = doctored();
    let id = runner_id();
    repo.run(&["suppress", &id, "--reason", reason, "--until", until]);
    let doctor = repo.run(&["doctor"]).envelope();
    (id, doctor)
}

#[test]
fn an_expired_date_reactivates_the_finding_and_reports_suppression_expired() {
    let (id, doctor) = doctor_after_suppressing("until new year", "2000-01-01");

    let reactivated = finding(&doctor, &id);
    assert_eq!(reactivated["suppressed"], json!(false));
    assert_eq!(reactivated["severity"], json!("warn"));
    assert!(kinds(&doctor).contains(&"suppression.expired".to_owned()));
    let expiry_id =
        FindingId::for_subject(&CheckId::from("suppress"), "suppression.expired", &id).to_string();
    let expired = finding(&doctor, &expiry_id);
    assert_eq!(
        expired["evidence"],
        json!({"finding": id, "reason": "until new year", "until": "2000-01-01"})
    );
    assert_eq!(doctor["summary"]["suppressed"], json!(0));
}

#[test]
fn a_future_date_keeps_the_suppression_active() {
    let (id, doctor) = doctor_after_suppressing("x", "2999-12-31");
    assert_eq!(finding(&doctor, &id)["suppressed"], json!(true));
    assert!(!kinds(&doctor).contains(&"suppression.expired".to_owned()));
}

#[test]
fn suppressing_every_fail_finding_turns_doctors_exit_3_into_0() {
    let repo = Repo::new();
    repo.write("cupel.toml", "required_version = \">=99.0\"\n");
    let first = repo.run_with(cupel().env("PATH", ""), &["doctor"]);
    assert_eq!(first.code, 3, "{}", first.stdout);
    let id = FindingId::for_subject(
        &CheckId::from("doctor"),
        "doctor.version-unsatisfied",
        "required_version",
    )
    .to_string();
    repo.run(&["suppress", &id, "--reason", "upgrading next sprint"]);

    let second = repo.run_with(cupel().env("PATH", ""), &["doctor"]);

    assert_eq!(second.code, 0, "{}", second.stdout);
    let envelope = second.envelope();
    assert_eq!(envelope["status"], json!("pass"));
    assert_eq!(finding(&envelope, &id)["severity"], json!("info"));
}

#[test]
fn a_malformed_suppressions_file_is_a_usage_error_naming_it() {
    let repo = doctored();
    repo.write(".cupel/suppressions.toml", "[[suppression]]\nfinding = 3\n");
    let outcome = repo.run(&["doctor"]);
    assert_eq!(outcome.code, 2, "{}", outcome.stdout);
    assert_eq!(
        outcome.finding()["evidence"]["argument"],
        json!(".cupel/suppressions.toml")
    );
}

fn git(dir: &std::path::Path, args: &[&str]) -> bool {
    Process::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn a_ref_expires_the_suppression_once_head_contains_it() {
    let repo = doctored();
    let root = repo.root();
    if !git(&root, &["init", "--quiet"]) {
        eprintln!("SKIPPED: git not installed");
        return;
    }
    assert!(git(&root, &["add", "cupel.toml"]));
    assert!(git(&root, &["commit", "--quiet", "-m", "first"]));
    assert!(git(&root, &["tag", "v1"]));
    let id = runner_id();

    repo.run(&["suppress", &id, "--reason", "x", "--until", "v9"]);
    let unresolved = repo.run(&["doctor"]).envelope();
    assert_eq!(finding(&unresolved, &id)["suppressed"], json!(true));

    repo.run(&["suppress", &id, "--reason", "x", "--until", "v1"]);
    let merged = repo.run(&["doctor"]).envelope();
    assert_eq!(finding(&merged, &id)["suppressed"], json!(false));
    assert!(kinds(&merged).contains(&"suppression.expired".to_owned()));
}
