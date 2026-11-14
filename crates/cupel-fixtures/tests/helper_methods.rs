// ABOUTME: Go tests that hand their `t` to helpers end to end: a test asserting only through a method of a
// ABOUTME: non-test file asserts as the method does, and tests whose helpers only set up assert nothing (V5).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_test_asserts_through_a_method_it_hands_t_and_not_through_a_setup_helper() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("helper-methods/go", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &["check", "--only", "tests.quality", "--format", "json"],
        dir.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let envelope = outcome.envelope();
    let unasserted: Vec<&str> = envelope
        .findings
        .iter()
        .filter(|finding| finding.kind == "test.no-assertion")
        .filter_map(|finding| finding.symbol.as_ref())
        .map(|symbol| symbol.display.as_str())
        .collect();
    assert_eq!(
        unasserted,
        [
            "fuzzy.TestTheClusterOnlyStops",
            "fuzzy.TestAScratchDirectoryOnly"
        ]
    );
    outcome.assert_golden("helper-methods/go");
}
