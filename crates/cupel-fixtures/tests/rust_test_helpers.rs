// ABOUTME: Rust tests that check end to end through `unwrap_err`, `expect_err`, or a helper of a sibling module
// ABOUTME: called by path assert, and ones that only unwrap or only call a setup function assert nothing (V18, V19).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_test_asserts_through_unwrap_err_or_a_module_helper_and_not_through_unwrap_or_setup() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("rust-test-helpers/rust", dir.path());
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
            "tests::suite::errors::digits_parse",
            "tests::suite::values::inputs_are_built"
        ]
    );
    outcome.assert_golden("rust-test-helpers/rust");
}
