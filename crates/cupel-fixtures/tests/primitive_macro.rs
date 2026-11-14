// ABOUTME: A Rust macro named like a primitive type end to end: a test naming its expected text with `str!` parses,
// ABOUTME: so Cupel reports no `lang.parse-error` and reads its assertions (V17, ADR-213).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_test_using_a_str_macro_is_no_parse_error() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("primitive-macro/rust", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &[
            "check",
            "--only",
            "tests.assertion-density",
            "--format",
            "json",
        ],
        dir.path(),
    );
    let envelope = outcome.envelope();
    let findings: Vec<(&str, &str)> = envelope
        .findings
        .iter()
        .map(|finding| {
            let file = finding.location.as_ref().map_or("", |l| l.file.as_str());
            (finding.kind.as_str(), file)
        })
        .collect();
    assert_eq!(findings, [], "two tests, one strong assertion each");
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
}
