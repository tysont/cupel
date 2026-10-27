// ABOUTME: Rust methods of impls for generic types named by a path end to end: the clone pair of two such impls
// ABOUTME: names each method on its own type, not as a function of the module (P9.9, ADR-138 §8).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn methods_of_impls_for_generic_types_named_by_a_path_are_named_on_their_types() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("impl-paths/rust", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &["check", "--only", "static.clones", "--format", "json"],
        dir.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let envelope = outcome.envelope();
    let titles: Vec<&str> = envelope
        .findings
        .iter()
        .map(|finding| finding.title.as_str())
        .collect();
    assert_eq!(
        titles,
        [
            "`IntoIter::next_if` and `Drain::next_if` (src/lib.rs:17) are type-1 clones (jaccard 1.00)"
        ]
    );
    outcome.assert_golden("impl-paths/rust");
}
