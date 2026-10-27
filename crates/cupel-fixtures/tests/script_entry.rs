// ABOUTME: A JavaScript or TypeScript script nothing imports runs its top-level code end to end: the functions that
// ABOUTME: code calls, or hands to a call as callbacks, are live, and one nothing calls is dead (P9.9 group 3).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn functions_a_script_calls_at_top_level_are_live() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("script-entry/typescript", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &["check", "--only", "static.dead-code", "--format", "json"],
        dir.path(),
    );
    let envelope = outcome.envelope();
    let findings: Vec<(&str, &str)> = envelope
        .findings
        .iter()
        .map(|finding| (finding.kind.as_str(), finding.title.as_str()))
        .collect();
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].0, "dead.symbol");
    assert!(
        findings[0].1.contains("scripts.report.unused"),
        "{findings:?}"
    );
    outcome.assert_golden("script-entry/typescript");
}
