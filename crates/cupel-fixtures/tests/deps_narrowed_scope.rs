// ABOUTME: `static.deps` on a narrowed scope end to end: a trait-only module whose one importer is outside the
// ABOUTME: scope keeps that importer in its afferent coupling, so it is not reported unstable (P9.9, ADR-052).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn an_importer_outside_a_narrowed_scope_counts_toward_a_modules_afferent_coupling() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("deps-narrowed-scope/rust", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &[
            "check",
            "--paths",
            "src/value/index.rs",
            "--only",
            "static.deps",
            "--format",
            "json",
        ],
        dir.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let envelope = outcome.envelope();
    let findings: Vec<&str> = envelope
        .findings
        .iter()
        .map(|finding| finding.kind.as_str())
        .collect();
    assert_eq!(
        findings,
        Vec::<&str>::new(),
        "src/value/mod.rs imports the trait module: afferent 1, efferent 2, distance 0.67"
    );
    outcome.assert_golden("deps-narrowed-scope/rust");
}
