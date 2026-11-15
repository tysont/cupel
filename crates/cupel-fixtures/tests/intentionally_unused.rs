// ABOUTME: Rust's leading-underscore convention end to end: a `_`-named function exists for the compiler and is
// ABOUTME: neither dead code nor uncovered, while a private function nothing calls is both (V21, ADR-226).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

/// The findings of `check` over a git-initialized copy of the fixture, as (kind, symbol). The
/// fixture's committed coverage report is fresh there, so `tests.coverage` runs no tool.
fn findings(check: &str) -> Vec<(String, String)> {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("intentionally-unused/rust", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(&["check", "--only", check, "--format", "json"], dir.path());
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
fn a_function_named_with_a_leading_underscore_is_not_dead_but_an_uncalled_one_is() {
    assert_eq!(
        findings("static.dead-code"),
        [("dead.symbol".to_owned(), "unused_length".to_owned())]
    );
}

#[test]
fn a_function_named_with_a_leading_underscore_is_not_uncovered_but_an_uncalled_one_is() {
    assert_eq!(
        findings("tests.coverage"),
        [(
            "coverage.uncovered-symbol".to_owned(),
            "unused_length".to_owned()
        )]
    );
}
