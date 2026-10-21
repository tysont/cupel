// ABOUTME: Integration tests for `cupel doctor` (ADR-022): version, configuration, `.cupel/` access,
// ABOUTME: and tool findings, exit 3 for anything required, and `--fix` and `--dry-run`.
mod support;

use std::process::Command as Process;

use cupel_core::{CheckId, FindingId};
use serde_json::{Value, json};
use support::{Outcome, Repo, cupel};

/// The ID ADR-016 assigns a doctor finding about `subject`.
fn doctor_id(kind: &str, subject: &str) -> Value {
    json!(FindingId::for_subject(&CheckId::from("doctor"), kind, subject).to_string())
}

fn git_installed() -> bool {
    Process::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Asserts `outcome` exited 3 with exactly one finding, of `kind`, and returns it.
fn assert_environment(outcome: &Outcome, kind: &str) -> Value {
    assert_eq!(outcome.code, 3, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("error"));
    let findings: Vec<&Value> = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["kind"] != json!("doctor.tool-missing"))
        .collect();
    assert_eq!(findings.len(), 1, "{findings:#?}");
    let finding = findings[0].clone();
    assert_eq!(finding["check"], json!("doctor"));
    assert_eq!(finding["kind"], json!(kind));
    assert_eq!(finding["severity"], json!("fail"));
    assert_eq!(finding["tier"], json!(0));
    assert_eq!(finding["symbol"], Value::Null);
    finding
}

#[test]
fn doctor_on_a_healthy_repository_passes() {
    let outcome = Repo::new().run(&["doctor"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("pass"));
    if git_installed() {
        assert_eq!(envelope["findings"], json!([]));
    } else {
        eprintln!("SKIPPED: git not installed; only the clean-findings assertion is skipped");
    }
}

/// `cupel doctor`'s outcome in a fresh repository holding `file` with `text`.
fn doctor_with(file: &str, text: &str) -> Outcome {
    let repo = Repo::new();
    repo.write(file, text);
    repo.run(&["doctor"])
}

/// The envelope of `cupel doctor` in a fresh repository holding `file` with `text`, asserting
/// it passes.
fn passing_doctor_with(file: &str, text: &str) -> Value {
    let outcome = doctor_with(file, text);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    outcome.envelope()
}

#[test]
fn an_unsatisfied_required_version_exits_3_asking_to_install_cupel() {
    let finding = assert_environment(
        &doctor_with("cupel.toml", "required_version = \">=99.0\"\n"),
        "doctor.version-unsatisfied",
    );
    assert_eq!(finding["action"]["kind"], json!("install_tool"));
    assert_eq!(
        finding["evidence"],
        json!({"required": ">=99.0", "actual": env!("CARGO_PKG_VERSION")})
    );
    assert_eq!(
        finding["id"],
        doctor_id("doctor.version-unsatisfied", "required_version")
    );
    assert_eq!(
        finding["action"]["summary"],
        json!(format!(
            "Install a cupel matching `>=99.0`, or change required_version. Live install routes: {}.",
            install_commands(true).join("; ")
        ))
    );
}

/// The commands of the install routes `cupel describe` lists as live (or not), each in backticks.
fn install_commands(live: bool) -> Vec<String> {
    support::described(&[])["install_routes"]
        .as_array()
        .expect("install_routes")
        .iter()
        .filter(|route| route["live"] == json!(live))
        .map(|route| format!("`{}`", route["command"].as_str().expect("command")))
        .collect()
}

/// An agent whose binary is too old learns every route that installs another today, and none that
/// does not work yet (ADR-128).
#[test]
fn the_version_finding_names_every_live_install_route_and_no_other() {
    let finding = assert_environment(
        &doctor_with("cupel.toml", "required_version = \">=99.0\"\n"),
        "doctor.version-unsatisfied",
    );
    let summary = finding["action"]["summary"]
        .as_str()
        .expect("summary")
        .to_owned();
    let live = install_commands(true);
    let named: Vec<&String> = live
        .iter()
        .filter(|command| summary.contains(*command))
        .collect();
    let pending = install_commands(false);
    let misnamed: Vec<&String> = pending
        .iter()
        .filter(|command| summary.contains(*command))
        .collect();
    assert!(!live.is_empty() && !pending.is_empty());
    assert_eq!(named, live.iter().collect::<Vec<_>>());
    assert_eq!(misnamed, Vec::<&String>::new());
}

#[test]
fn a_satisfied_required_version_is_not_a_finding() {
    let envelope = passing_doctor_with("cupel.toml", "required_version = \">=0.1, <99\"\n");
    let kinds: Vec<&Value> = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| &finding["kind"])
        .filter(|kind| kind.as_str().unwrap().starts_with("doctor.version"))
        .collect();
    assert_eq!(kinds, Vec::<&Value>::new());
}

#[test]
fn a_malformed_required_version_exits_3_asking_to_fix_the_configuration() {
    let finding = assert_environment(
        &doctor_with("cupel.toml", "required_version = \"soon\"\n"),
        "doctor.version-invalid",
    );
    assert_eq!(finding["action"]["kind"], json!("configure"));
    assert_eq!(
        finding["id"],
        doctor_id("doctor.version-invalid", "required_version")
    );
}

#[test]
fn configuration_that_does_not_load_is_a_finding_not_a_usage_error() {
    let repo = Repo::new();
    repo.write("cupel.toml", "colour = \"blue\"\n");
    assert_eq!(repo.run(&["check"]).code, 2, "other verbs still refuse it");
    let finding = assert_environment(&repo.run(&["doctor"]), "doctor.config-invalid");
    assert_eq!(finding["action"]["kind"], json!("configure"));
    assert_eq!(
        finding["id"],
        doctor_id("doctor.config-invalid", "cupel.toml")
    );
    let title = finding["title"].as_str().unwrap();
    assert!(title.contains("colour"), "{title}");
}

#[test]
fn an_unknown_profile_is_reported_against_the_profile_flag() {
    let finding = assert_environment(
        &Repo::new().run(&["doctor", "--profile", "nightly"]),
        "doctor.config-invalid",
    );
    assert_eq!(finding["evidence"]["argument"], json!("--profile"));
    assert_eq!(
        finding["id"],
        doctor_id("doctor.config-invalid", "--profile")
    );
}

#[test]
fn a_cupel_path_that_is_a_file_is_reported_as_unwritable() {
    let outcome = doctor_with(".cupel", "");
    let finding = assert_environment(&outcome, "doctor.cupel-unwritable");
    assert_eq!(finding["action"]["kind"], json!("configure"));
    assert_eq!(
        finding["id"],
        doctor_id("doctor.cupel-unwritable", ".cupel")
    );
    assert!(finding["evidence"]["error"].is_string(), "{finding}");
}

#[cfg(unix)]
#[test]
fn a_read_only_cupel_directory_is_reported_as_unwritable() {
    use std::os::unix::fs::PermissionsExt;

    let repo = Repo::new();
    let dir = repo.path(".cupel");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let enforced = std::fs::write(dir.join("x"), "").is_err();
    let outcome = repo.run(&["doctor"]);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    if !enforced {
        eprintln!("SKIPPED: this user can write to a read-only directory (running as root?)");
        return;
    }
    assert_environment(&outcome, "doctor.cupel-unwritable");
}

#[test]
fn missing_git_is_a_warning_and_doctor_still_passes() {
    let outcome = Repo::new().run_with(cupel().env("PATH", ""), &["doctor"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("doctor.tool-missing"));
    assert_eq!(finding["severity"], json!("warn"));
    assert_eq!(finding["action"]["kind"], json!("install_tool"));
    assert_eq!(finding["id"], doctor_id("doctor.tool-missing", "git"));
}

#[test]
fn fix_with_nothing_installable_reports_no_installs() {
    for args in [&["doctor", "--fix"][..], &["doctor", "--fix", "--install"]] {
        let outcome = Repo::new().run_with(cupel().env("PATH", ""), args);
        assert_eq!(outcome.code, 0, "{}", outcome.stdout);
        let kinds: Vec<Value> = outcome.envelope()["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|finding| finding["kind"].clone())
            .collect();
        assert_eq!(kinds, [json!("doctor.tool-missing")], "{args:?}");
    }
}

#[test]
fn a_dry_run_plans_nothing_and_writes_nothing() {
    let repo = Repo::new();
    let outcome = repo.run(&["doctor", "--dry-run"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("plan"));
    assert_eq!(envelope["plan"]["would_write"], json!([]));
    assert!(repo.tree().is_empty(), "{:?}", repo.tree());
}

#[test]
fn doctor_describes_fix_as_implemented() {
    let envelope = Repo::new().run(&["doctor", "--describe"]).envelope();
    let description = &envelope["description"];
    assert_eq!(description["pending_step"], Value::Null);
    assert_eq!(description["arguments"][0]["name"], json!("--fix"));
    assert_eq!(description["arguments"][0]["pending_step"], Value::Null);
}

#[test]
fn a_configured_runner_that_is_not_installed_warns_without_failing() {
    let envelope = passing_doctor_with(
        "cupel.toml",
        "[lang.rust]\ntest = \"cupel-test-no-such-runner --all\"\n",
    );
    let finding = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["evidence"]["tool"] == json!("cupel-test-no-such-runner"))
        .unwrap_or_else(|| panic!("{envelope}"))
        .clone();
    assert_eq!(finding["kind"], json!("doctor.tool-missing"));
    assert_eq!(finding["severity"], json!("warn"));
    assert_eq!(finding["action"]["kind"], json!("install_tool"));
    assert_eq!(finding["evidence"]["needed_for"], json!("[lang.rust] test"));
    assert_eq!(
        finding["id"],
        doctor_id("doctor.tool-missing", "cupel-test-no-such-runner")
    );
}

/// Writes the slot directory `name` under `tmp`'s `cupel-mutate/` for the repository at
/// `repository`, holding `bytes` in one slot.
fn slots(tmp: &std::path::Path, name: &str, repository: &str, bytes: usize) {
    let dir = tmp.join("cupel-mutate").join(name);
    std::fs::create_dir_all(dir.join("python/0")).unwrap();
    std::fs::write(dir.join("repository"), repository).unwrap();
    std::fs::write(dir.join("python/0/build.out"), vec![b'x'; bytes]).unwrap();
}

#[test]
fn doctor_reports_the_mutation_slots_and_the_repositories_they_belong_to() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = Repo::new();
    let kept = repo.root().display().to_string();
    slots(tmp.path(), "a", &kept, 2_048);
    slots(tmp.path(), "b", "/no/such/repository", 1_000);
    let outcome = repo.run_with(cupel().env("TMPDIR", tmp.path()), &["doctor"]);
    let envelope = outcome.envelope();
    let finding = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["kind"] == "doctor.mutation-slots")
        .unwrap_or_else(|| panic!("{envelope}"));
    assert_eq!(finding["severity"], "info");
    let evidence = &finding["evidence"];
    let total = evidence["bytes"].as_u64().unwrap();
    assert!((3_048..3_048 + 1_024).contains(&total), "{evidence}");
    let mut repositories: Vec<(Value, Value)> = evidence["repositories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|slots| (slots["repository"].clone(), slots["exists"].clone()))
        .collect();
    repositories.sort_by_key(|(path, _)| path.to_string());
    assert_eq!(
        repositories,
        [
            (json!("/no/such/repository"), json!(false)),
            (json!(kept), json!(true))
        ]
    );
    let title = finding["title"].as_str().unwrap();
    assert!(
        title.starts_with("Mutation slots hold 3.") && title.contains(" KiB for 2 repositories under "),
        "{title}"
    );
}

#[test]
fn doctor_says_nothing_of_mutation_slots_when_there_are_none() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = Repo::new();
    let outcome = repo.run_with(cupel().env("TMPDIR", tmp.path()), &["doctor"]);
    let kinds = support::finding_fields(&outcome.envelope(), "kind");
    assert!(!kinds.contains(&"doctor.mutation-slots".to_owned()), "{kinds:?}");
}
