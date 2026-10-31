// ABOUTME: Tests for `cupel_fixtures::own_tmpdir`: a test run again in a child of its test binary whose TMPDIR the call
// ABOUTME: owns, so what the tools it starts keep there goes when it ends, and a failing body fails the test.
use std::path::Path;
use std::process::Command;

use cupel_fixtures::own_tmpdir::{is_child, ran_in_own_tmpdir};
use tempfile::TempDir;

/// This binary's test `name`, run alone with `tmp` as its TMPDIR.
fn run_alone(name: &str, tmp: &Path) -> std::process::Output {
    Command::new(std::env::current_exe().unwrap())
        .args([name, "--exact", "--nocapture"])
        .env("TMPDIR", tmp)
        .output()
        .unwrap()
}

/// The names of what `dir` holds.
fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The body leaves a directory in its TMPDIR, as jest leaves its cache; the call removes it.
#[test]
fn a_body_that_leaves_a_directory() {
    if ran_in_own_tmpdir("a_body_that_leaves_a_directory") {
        return;
    }
    assert!(is_child());
    std::fs::create_dir(std::env::temp_dir().join("jest_left")).unwrap();
}

/// The body fails.
#[test]
fn a_body_that_fails() {
    if !is_child() && std::env::var_os("OWN_TMPDIR_FAIL").is_none() {
        return;
    }
    if ran_in_own_tmpdir("a_body_that_fails") {
        return;
    }
    panic!("the body failed");
}

#[test]
fn what_the_body_leaves_in_its_tmpdir_goes_with_the_call() {
    let tmp = TempDir::new().unwrap();
    let output = run_alone("a_body_that_leaves_a_directory", tmp.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(entries(tmp.path()), Vec::<String>::new());
}

#[test]
fn a_failing_body_fails_its_test_with_the_childs_output() {
    let tmp = TempDir::new().unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["a_body_that_fails", "--exact", "--nocapture"])
        .env("TMPDIR", tmp.path())
        .env("OWN_TMPDIR_FAIL", "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stdout}");
    assert!(stderr.contains("the body failed"), "{stderr}");
    assert_eq!(entries(tmp.path()), Vec::<String>::new());
}

/// A name that matches no test of the binary fails the call: the child would otherwise pass having
/// run nothing.
#[test]
#[should_panic(expected = "the child ran no test named no_such_test")]
fn a_name_that_matches_no_test_fails() {
    let _ = ran_in_own_tmpdir("no_such_test");
}
