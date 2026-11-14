// ABOUTME: Python imports under a TYPE_CHECKING guard end to end: they close no dependency cycle, while a cycle of
// ABOUTME: imports that run beside them is still `dep.cycle` (V13, ADR-211).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_cycle_closed_only_by_type_checking_imports_is_no_dependency_cycle() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("type-checking-imports/python", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &["check", "--only", "static.deps", "--format", "json"],
        dir.path(),
    );
    let envelope = outcome.envelope();
    let cycles: Vec<&str> = envelope
        .findings
        .iter()
        .filter(|finding| finding.kind == "dep.cycle")
        .map(|finding| finding.title.as_str())
        .collect();
    assert_eq!(
        cycles,
        ["Import cycle of 2 modules: relay/_left.py → relay/_right.py → relay/_left.py"],
        "the other two cycles close through imports under `if TYPE_CHECKING:`"
    );
}
