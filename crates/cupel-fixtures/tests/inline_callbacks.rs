// ABOUTME: A return that ends an inline callback is not an early exit of the function that passes it, end to end:
// ABOUTME: a test of sixteen such callbacks is not cognitively complex (P9.9 group 3, ts-pattern).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn returns_that_end_inline_callbacks_add_no_cognitive_complexity() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("inline-callbacks/typescript", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &["check", "--only", "static.complexity", "--format", "json"],
        dir.path(),
    );
    let envelope = outcome.envelope();
    let kinds: Vec<&str> = envelope
        .findings
        .iter()
        .map(|finding| finding.kind.as_str())
        .collect();
    assert_eq!(
        kinds,
        ["function.long"],
        "only its 70 lines are over a limit"
    );
    outcome.assert_golden("inline-callbacks/typescript");
}
