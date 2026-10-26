// ABOUTME: TypeScript's type-level assertions end to end: `Expect<Equal<A, B>>` and `// @ts-expect-error` are
// ABOUTME: assertions of the case that holds them, so only the case without either asserts nothing (P9.9 group 3).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn type_level_assertions_are_assertions_of_their_case() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("type-assertions/typescript", dir.path());
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
    assert_eq!(
        findings,
        [("test.weak-assertion", 22), ("test.no-assertion", 27)],
        "the two `Expect<Equal<…>>` cases pin a type; `@ts-expect-error` is a weak assertion"
    );
    outcome.assert_golden("type-assertions/typescript");
}
