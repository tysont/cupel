// ABOUTME: Setup loops end to end over Go tests: a loop that only builds inputs is no test.logic, one that
// ABOUTME: branches while building is, and one asserting on each pass is table-driven (ADR-220).
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_loop_building_inputs_is_no_logic_but_a_loop_that_branches_is() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("setup-loops/go", dir.path());
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
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let envelope = outcome.envelope();
    let logic: Vec<(&str, &serde_json::Value)> = envelope
        .findings
        .iter()
        .filter(|finding| finding.kind == "test.logic")
        .map(|finding| {
            (
                finding.symbol.as_ref().map_or("", |s| s.display.as_str()),
                &finding.evidence.0["setup"],
            )
        })
        .collect();
    assert_eq!(
        logic,
        [(
            "cluster.TestDecodeMakesEveryServerAVoter",
            &serde_json::json!(0)
        )]
    );
}
