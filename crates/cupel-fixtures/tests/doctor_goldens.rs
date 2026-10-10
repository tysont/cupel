// ABOUTME: Golden envelopes for `cupel doctor`: a clean run on an empty repository, and exit 3 on a
// ABOUTME: repository whose `required_version` this binary cannot satisfy.
use std::process::Command;

use cupel_core::{ExitCode, Status};
use cupel_fixtures::{copy_repo, run_cupel};

fn git_installed() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn doctor_on_an_empty_repository_matches_its_golden() {
    if !git_installed() {
        eprintln!("SKIPPED: git not installed; doctor would add a doctor.tool-missing warning");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo("detect/empty", dir.path());

    let outcome = run_cupel(&["doctor", "--format", "json"], dir.path());

    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    assert_eq!(outcome.envelope().status, Status::Pass);
    outcome.assert_golden("doctor/empty");
}

#[test]
fn doctor_on_an_unsatisfiable_required_version_matches_its_golden() {
    if !git_installed() {
        eprintln!("SKIPPED: git not installed; doctor would add a doctor.tool-missing warning");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo("doctor/required-version", dir.path());

    let outcome = run_cupel(&["doctor", "--format", "json"], dir.path());

    assert_eq!(outcome.code, Some(3), "{}", outcome.stdout);
    assert_eq!(outcome.envelope().exit_code, ExitCode::Environment);
    outcome.assert_golden("doctor/required-version");
}
