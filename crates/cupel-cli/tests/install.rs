// ABOUTME: Integration tests for installing tools (ADR-081): `doctor --fix` with `--install`, `--no-install`,
// ABOUTME: and neither on a pipe; failed installs; dry runs' `would_install`; and a real pip install into a venv.
mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use support::{Outcome, Repo, cupel};
use tempfile::TempDir;

/// The command the toolchain table gives for pytest with the `pip` launcher.
const PIP_INSTALL: &str = "python -m pip install pytest";

/// A copy of the fixture whose pinned runner, pytest, is not on the `PATH` the tests give.
fn repo() -> Repo {
    let repo = Repo::new();
    cupel_fixtures::copy_repo("doctor/runner-missing", &repo.root());
    repo
}

/// A directory of programs that becomes the whole `PATH`: `sh` and `git` from this machine,
/// and any `scripts` written as executable shell scripts.
struct Bin {
    dir: TempDir,
}

impl Bin {
    fn new(scripts: &[(&str, &str)]) -> Self {
        let bin = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        for program in ["sh", "git"] {
            if let Some(path) = cupel_core::find_program(program) {
                std::os::unix::fs::symlink(path, bin.dir.path().join(program)).unwrap();
            }
        }
        for (name, body) in scripts {
            let path = bin.dir.path().join(name);
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        bin
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Runs `doctor`, or another verb, in `repo` with this directory as the only `PATH`.
    fn run(&self, repo: &Repo, args: &[&str]) -> Outcome {
        repo.run_with(cupel().env("PATH", self.path()), args)
    }
}

/// A `python` that records that it ran by creating `marker`, then fails.
fn recording_python(marker: &Path) -> String {
    format!("touch '{}'\nexit 1", marker.display())
}

/// The findings of `envelope` of `kind`.
fn of_kind<'a>(envelope: &'a Value, kind: &str) -> Vec<&'a Value> {
    envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["kind"] == json!(kind))
        .collect()
}

/// The only finding of `kind` in `envelope`.
fn only<'a>(envelope: &'a Value, kind: &str) -> &'a Value {
    let found = of_kind(envelope, kind);
    assert_eq!(found.len(), 1, "{kind}: {envelope:#}");
    found[0]
}

/// Asserts `outcome` is doctor's exit 3 with pytest still missing, and returns the envelope.
fn pytest_still_missing(outcome: &Outcome) -> Value {
    assert_eq!(outcome.code, 3, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(pytest_missing(&envelope)["severity"], json!("fail"));
    envelope
}

/// The `doctor.tool-missing` finding about pytest.
fn pytest_missing(envelope: &Value) -> &Value {
    of_kind(envelope, "doctor.tool-missing")
        .into_iter()
        .find(|finding| finding["evidence"]["tool"] == json!("pytest"))
        .unwrap_or_else(|| panic!("pytest is not missing: {envelope:#}"))
}

#[test]
fn doctor_names_the_install_command_of_a_missing_runner() {
    let bin = Bin::new(&[]);
    let envelope = pytest_still_missing(&bin.run(&repo(), &["doctor"]));
    let missing = pytest_missing(&envelope);
    assert_eq!(missing["evidence"]["install"], json!(PIP_INSTALL));
    let summary = missing["action"]["summary"].as_str().unwrap();
    assert!(summary.contains(PIP_INSTALL), "{summary}");
    assert!(
        summary.contains("cupel doctor --fix --install"),
        "{summary}"
    );
    for kind in ["install.succeeded", "install.failed", "install.skipped"] {
        assert!(of_kind(&envelope, kind).is_empty(), "{kind}");
    }
}

#[test]
fn fix_with_no_install_leaves_the_tool_missing_and_runs_nothing() {
    let scratch = tempfile::tempdir().unwrap();
    let marker = scratch.path().join("ran");
    let bin = Bin::new(&[("python", &recording_python(&marker))]);
    let envelope = pytest_still_missing(&bin.run(&repo(), &["doctor", "--fix", "--no-install"]));
    let skipped = only(&envelope, "install.skipped");
    assert_eq!(skipped["check"], json!("install"));
    assert_eq!(skipped["severity"], json!("info"));
    assert_eq!(
        skipped["evidence"],
        json!({"tool": "pytest", "command": PIP_INSTALL, "reason": "--no-install"})
    );
    assert!(!marker.exists(), "the install ran");
}

#[test]
fn fix_on_a_pipe_without_a_flag_does_not_install() {
    let scratch = tempfile::tempdir().unwrap();
    let marker = scratch.path().join("ran");
    let bin = Bin::new(&[("python", &recording_python(&marker))]);
    let envelope = pytest_still_missing(&bin.run(&repo(), &["doctor", "--fix"]));
    let reason = only(&envelope, "install.skipped")["evidence"]["reason"].clone();
    assert!(
        reason.as_str().unwrap().contains("not a terminal"),
        "{reason}"
    );
    assert!(!marker.exists(), "the install ran");
}

#[test]
fn a_failed_install_fails_with_the_command_and_the_tail_of_its_stderr() {
    let bin = Bin::new(&[(
        "python",
        "echo 'Collecting pytest'\necho 'ERROR: no network here' >&2\nexit 1",
    )]);
    let envelope = pytest_still_missing(&bin.run(&repo(), &["doctor", "--fix", "--install"]));
    let failed = only(&envelope, "install.failed");
    assert_eq!(failed["severity"], json!("fail"));
    assert_eq!(failed["action"]["kind"], json!("install_tool"));
    assert_eq!(
        failed["evidence"],
        json!({"tool": "pytest", "command": PIP_INSTALL, "exit_code": 1,
               "stderr": "ERROR: no network here"})
    );
}

#[test]
fn an_install_that_succeeds_without_providing_the_tool_fails() {
    let bin = Bin::new(&[("python", "exit 0")]);
    let envelope = pytest_still_missing(&bin.run(&repo(), &["doctor", "--fix", "--install"]));
    let failed = only(&envelope, "install.failed");
    assert_eq!(failed["evidence"]["exit_code"], json!(0));
    let title = failed["title"].as_str().unwrap();
    assert!(title.contains("still not found"), "{title}");
}

#[test]
fn a_dry_run_of_fix_lists_the_installs_it_would_make_and_makes_none() {
    let scratch = tempfile::tempdir().unwrap();
    let marker = scratch.path().join("ran");
    let bin = Bin::new(&[("python", &recording_python(&marker))]);
    let repo = repo();
    let planned = |args: &[&str]| {
        let outcome = bin.run(&repo, args);
        assert_eq!(outcome.code, 0, "{}", outcome.stdout);
        let envelope = outcome.envelope();
        assert_eq!(envelope["status"], json!("plan"));
        envelope["plan"]["would_install"].clone()
    };
    assert_eq!(
        planned(&["doctor", "--fix", "--install", "--dry-run"]),
        json!([{"tool": "pytest", "command": PIP_INSTALL}])
    );
    assert_eq!(planned(&["doctor", "--fix", "--dry-run"]), json!([]));
    assert_eq!(planned(&["doctor", "--install", "--dry-run"]), json!([]));
    assert!(!marker.exists(), "the install ran");
}

#[test]
fn a_dry_run_of_check_lists_the_installs_install_would_make() {
    let bin = Bin::new(&[]);
    let repo = repo();
    let planned = |args: &[&str]| {
        let outcome = bin.run(&repo, args);
        assert_eq!(outcome.code, 0, "{}", outcome.stdout);
        outcome.envelope()["plan"]["would_install"].clone()
    };
    assert_eq!(
        planned(&["check", "--dry-run", "--install"]),
        json!([{"tool": "pytest", "command": PIP_INSTALL}])
    );
    assert_eq!(planned(&["check", "--dry-run"]), json!([]));
    assert_eq!(planned(&["check", "--dry-run", "--no-install"]), json!([]));
}

#[test]
fn check_with_install_installs_before_it_runs_and_reports_a_failure() {
    let bin = Bin::new(&[("python", "echo 'ERROR: no network here' >&2\nexit 1")]);
    let outcome = bin.run(
        &repo(),
        &["check", "--install", "--only", "tests.flaky", "--no-gate"],
    );
    let envelope = outcome.envelope();
    let failed = only(&envelope, "install.failed");
    assert_eq!(failed["evidence"]["command"], json!(PIP_INSTALL));
    let skipped = &envelope["checks"][0];
    assert_eq!(skipped["id"], json!("tests.flaky"), "{envelope:#}");
    assert_eq!(skipped["skipped"]["reason"], json!("unavailable"));
}

/// A fresh virtual environment in a scratch directory, made by this machine's `python3`;
/// `None` (reported as skipped) when there is no `python3` or it cannot make one.
fn scratch_venv() -> Option<(TempDir, PathBuf)> {
    if cupel_core::find_program("python3").is_none() {
        eprintln!("SKIPPED: python3 not installed; no virtual environment to install pytest into");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let venv = dir.path().join("venv");
    let made = std::process::Command::new("python3")
        .args(["-m", "venv"])
        .arg(&venv)
        .output()
        .is_ok_and(|output| output.status.success());
    if !made {
        eprintln!("SKIPPED: python3 -m venv failed; no virtual environment to install pytest into");
        return None;
    }
    Some((dir, venv))
}

#[test]
fn fix_with_install_installs_the_runner_into_a_scratch_venv_and_passes() {
    let Some((_scratch, venv)) = scratch_venv() else {
        return;
    };
    let bin = Bin::new(&[]);
    let path = std::env::join_paths([venv.join("bin"), bin.path().to_path_buf()]).unwrap();
    let repo = repo();
    let outcome = repo.run_with(
        // The developer's pip configuration (`user = true`, say) must not reach the install.
        cupel()
            .env("PATH", &path)
            .env("VIRTUAL_ENV", &venv)
            .env("PIP_CONFIG_FILE", "/dev/null")
            .env("PIP_DISABLE_PIP_VERSION_CHECK", "1"),
        &["doctor", "--fix", "--install"],
    );
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    let installed = only(&envelope, "install.succeeded");
    assert_eq!(installed["evidence"]["command"], json!(PIP_INSTALL));
    let version = envelope["run"]["toolchain"]["pytest"].as_str().unwrap();
    assert!(version.starts_with("pytest "), "{version}");
    assert_eq!(installed["evidence"]["version"], json!(version));
    assert!(
        of_kind(&envelope, "doctor.tool-missing")
            .iter()
            .all(|finding| finding["evidence"]["tool"] != json!("pytest"))
    );
    assert!(venv.join("bin").join("pytest").is_file());
}
