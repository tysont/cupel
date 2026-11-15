// ABOUTME: Python memberships end to end: a test asserting `a in b` asserts strongly and is not reported, one
// ABOUTME: asserting only `a not in b` is test.weak-assertion (ADR-223).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_membership_is_strong_and_a_negated_one_weak() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("membership-assertions/python", dir.path());
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
    let envelope = outcome.envelope();
    let findings: Vec<(&str, &str)> = envelope
        .findings
        .iter()
        .map(|finding| {
            let symbol = finding.symbol.as_ref().map_or("", |s| s.display.as_str());
            (finding.kind.as_str(), symbol)
        })
        .collect();
    assert_eq!(
        findings,
        [(
            "test.weak-assertion",
            "tests.test_auth.test_no_cookie_is_sent"
        )]
    );
}
