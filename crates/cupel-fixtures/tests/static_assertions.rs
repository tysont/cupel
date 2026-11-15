// ABOUTME: Compile-time assertions end to end: Go's `var _ I = &T{}` and Rust's bounded local function and
// ABOUTME: `const _: () = assert!(…)` in a test assert strongly, while a test that only builds asserts nothing (ADR-222).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

/// The `tests.quality` findings over a git-initialized copy of `repo`, as (kind, test).
fn quality(repo: &str) -> Vec<(String, String)> {
    let dir = tempfile::tempdir().unwrap();
    copy_repo(repo, dir.path());
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
    outcome
        .envelope()
        .findings
        .iter()
        .map(|finding| {
            let symbol = finding.symbol.as_ref().map_or("", |s| s.display.as_str());
            (finding.kind.clone(), symbol.to_owned())
        })
        .collect()
}

#[test]
fn a_go_conformance_test_asserts_and_a_test_that_only_builds_does_not() {
    assert_eq!(
        quality("static-assertions/go"),
        [("test.no-assertion".to_owned(), "lru.TestNewBuilds".to_owned())]
    );
}

#[test]
fn rust_compile_time_checks_assert_and_a_test_that_only_builds_does_not() {
    assert_eq!(
        quality("static-assertions/rust"),
        [(
            "test.no-assertion".to_owned(),
            "tests::a_group_builds".to_owned()
        )]
    );
}
