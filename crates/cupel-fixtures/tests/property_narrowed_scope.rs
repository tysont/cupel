// ABOUTME: `gen.property` on a scope narrowed to a source end to end: the source's property tests are outside the
// ABOUTME: scope but still its tests, so its functions are not reported missing a property (P9.9, ADR-096).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_source_whose_property_tests_are_outside_the_scope_lacks_no_property() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("gen.property/rust", dir.path());
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
            "src/digits.rs",
            "--only",
            "gen.property",
            "--no-install",
            "--format",
            "json",
        ],
        dir.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let envelope = outcome.envelope();
    let missing: Vec<&str> = envelope
        .findings
        .iter()
        .filter(|finding| finding.kind == "property.missing")
        .map(|finding| finding.title.as_str())
        .collect();
    assert_eq!(
        missing,
        Vec::<&str>::new(),
        "tests/digits.rs holds digits' properties"
    );
}
