// ABOUTME: JUnit's expected-exception idiom end to end: a `try` ending in `fail` with a catch is a throws
// ABOUTME: assertion whose catch is no test.logic, while a catch of a `try` that does not fail is (V15, ADR-212).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_try_ending_in_fail_is_an_expected_exception_and_another_catch_is_logic() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("expected-exception/java", dir.path());
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
            "test.logic",
            "codec.DecoderTest.decodeCountsLettersOrExplains"
        )],
        "the two tests whose `try` ends in `fail` assert the exception"
    );
}
