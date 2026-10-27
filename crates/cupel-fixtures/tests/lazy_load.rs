// ABOUTME: A Clojure namespace loaded by `requiring-resolve` through a macro is reachable end to end: nothing of
// ABOUTME: it is dead code, though no `:require` names it (P9.9 group 3, cljfx's lazy registry).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_lazily_loaded_namespace_is_not_dead_code() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("lazy-load/clojure", dir.path());
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
            let file = finding.location.as_ref().map_or("", |l| l.file.as_str());
            (finding.kind.as_str(), file)
        })
        .collect();
    assert_eq!(findings, [], "{}", outcome.stdout);
    outcome.assert_golden("lazy-load/clojure");
}
