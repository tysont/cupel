// ABOUTME: Rust's leading-underscore convention end to end: a `_`-named function exists for the compiler and is no
// ABOUTME: dead code, while a private function nothing calls is (V21).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_function_named_with_a_leading_underscore_is_not_dead_but_an_uncalled_one_is() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("intentionally-unused/rust", dir.path());
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
        .map(|finding| {
            let symbol = finding.symbol.as_ref().map_or("", |s| s.display.as_str());
            (finding.kind.as_str(), symbol)
        })
        .collect();
    assert_eq!(findings, [("dead.symbol", "unused_length")]);
}
