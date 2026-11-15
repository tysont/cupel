// ABOUTME: Guarded failures end to end: Go's `if <predicate> { t.Fatal }` and Rust's `if <cond> { panic! }` are
// ABOUTME: strong assertions, an `err` guard stays weak, and a Rust `if let` panic asserts nothing (ADR-221).
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

fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(kind, symbol)| ((*kind).to_owned(), (*symbol).to_owned()))
        .collect()
}

#[test]
fn a_go_failure_under_a_predicate_is_strong_and_one_under_an_err_guard_is_weak() {
    assert_eq!(
        quality("guarded-failures/go"),
        pairs(&[("test.weak-assertion", "votes.TestParseAcceptsAnID")])
    );
}

#[test]
fn a_rust_panic_under_a_condition_is_strong_and_one_under_an_if_let_asserts_nothing() {
    assert_eq!(
        quality("guarded-failures/rust"),
        pairs(&[
            ("test.logic", "tests::votes::parsing_an_id_succeeds"),
            ("test.no-assertion", "tests::votes::parsing_an_id_succeeds"),
        ]),
        "the `if let` guards no assertion, so it is a branch of logic"
    );
}
