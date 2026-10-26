// ABOUTME: Rust properties in a quickcheck! block end to end: each is a test, and a property that asserts through
// ABOUTME: a helper of its file asserts, so its file is not sparse beside plain tests (P9.9, ADR-095).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn properties_in_a_quickcheck_block_assert_through_a_helper_of_their_file() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("quickcheck-block/rust", dir.path());
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
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let envelope = outcome.envelope();
    let findings: Vec<(&str, &str)> = envelope
        .findings
        .iter()
        .map(|finding| {
            let file = finding.location.as_ref().map_or("", |l| l.file.as_str());
            (finding.kind.as_str(), file)
        })
        .collect();
    assert_eq!(findings, [], "six tests, one strong assertion each");
    outcome.assert_golden("quickcheck-block/rust");
}
