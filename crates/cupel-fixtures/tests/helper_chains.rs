// ABOUTME: Rust assert_cmd chains end to end: a chain that starts with a call named like an asserting helper (of
// ABOUTME: another test file, or of its own) keeps its strong process assertion, so no test reads as weak (P9.9).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_chain_that_starts_with_a_call_named_like_an_asserting_helper_stays_a_process_assertion() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("helper-chains/rust", dir.path());
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
    let findings: Vec<(&str, &str)> = envelope
        .findings
        .iter()
        .map(|finding| (finding.kind.as_str(), finding.title.as_str()))
        .collect();
    assert_eq!(findings, Vec::<(&str, &str)>::new());
    outcome.assert_golden("helper-chains/rust");
}
