// ABOUTME: Type-level tests whose declared types differ are not clones, end to end: a type declaration keeps its
// ABOUTME: type arguments when it normalizes, so the tests compare by what they check (P9.9 group 3, ts-pattern).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn type_level_tests_of_different_types_are_not_clones() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("type-clones/typescript", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &["check", "--only", "static.clones", "--format", "json"],
        dir.path(),
    );
    let envelope = outcome.envelope();
    let kinds: Vec<&str> = envelope
        .findings
        .iter()
        .map(|finding| finding.kind.as_str())
        .collect();
    assert_eq!(kinds, [] as [&str; 0], "{}", outcome.stdout);
    outcome.assert_golden("type-clones/typescript");
}
