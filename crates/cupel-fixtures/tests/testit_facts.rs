// ABOUTME: Clojure tests that assert with metosin/testit's `fact` and `facts` end to end: each arrow is an
// ABOUTME: assertion of its `deftest`, so only the test without one asserts nothing (P9.9 group 3, cljfx).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn testit_facts_are_assertions_of_their_test() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("testit-facts/clojure", dir.path());
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
    let findings: Vec<(&str, u32)> = envelope
        .findings
        .iter()
        .map(|finding| {
            let line = finding.location.as_ref().map_or(0, |l| l.line);
            (finding.kind.as_str(), line)
        })
        .collect();
    assert_eq!(findings, [("test.no-assertion", 17)], "{}", outcome.stdout);
    outcome.assert_golden("testit-facts/clojure");
}
