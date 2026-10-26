// ABOUTME: `test.duplicate-setup` end to end over Rust tests: leading assertions are not setup, so tests that open
// ABOUTME: with assertions on different functions are not reported, while tests that repeat their arrange code are.
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn leading_assertions_are_not_setup_but_repeated_arrange_code_is() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("setup-assertions/rust", dir.path());
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
    let duplicated: Vec<&str> = envelope
        .findings
        .iter()
        .filter(|finding| finding.kind == "test.duplicate-setup")
        .filter_map(|finding| finding.symbol.as_ref())
        .map(|symbol| symbol.display.as_str())
        .collect();
    assert_eq!(duplicated, ["tests::masks::sum_test"]);
    outcome.assert_golden("setup-assertions/rust");
}
