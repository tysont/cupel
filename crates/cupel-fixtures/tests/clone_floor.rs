// ABOUTME: static.clones' size floor end to end (ADR-158): two 22-node builder setters fall below the default
// ABOUTME: `min_nodes` and do not pair, two 35-node single-`match` accessors still pair; at 20 the setters pair too.
use std::path::Path;
use std::process::Command;

use cupel_fixtures::{Outcome, copy_repo, run_cupel};

/// A git copy of the fixture, with `config` as its `cupel.toml` when given.
fn fixture(config: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("clone-floor/rust", dir.path());
    if let Some(config) = config {
        std::fs::write(dir.path().join("cupel.toml"), config).unwrap();
    }
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    dir
}

fn clones(dir: &Path) -> Outcome {
    run_cupel(
        &["check", "--only", "static.clones", "--format", "json"],
        dir,
    )
}

/// Each group's members as (symbol, nodes).
fn groups(outcome: &Outcome) -> Vec<Vec<(String, u64)>> {
    outcome
        .envelope()
        .findings
        .iter()
        .map(|finding| {
            let evidence = &finding.evidence.0;
            [&evidence["left"], &evidence["right"]]
                .iter()
                .map(|member| {
                    (
                        member["symbol"].as_str().unwrap().to_owned(),
                        member["nodes"].as_u64().unwrap(),
                    )
                })
                .collect()
        })
        .collect()
}

fn pair(left: &str, right: &str, nodes: u64) -> Vec<(String, u64)> {
    vec![(left.to_owned(), nodes), (right.to_owned(), nodes)]
}

#[test]
fn a_22_node_setter_pair_falls_below_the_floor_and_a_35_node_match_pair_does_not() {
    let dir = fixture(None);
    let outcome = clones(dir.path());
    assert_eq!(
        groups(&outcome),
        [pair("OracleKind::as_str", "Phase::as_str", 35)],
        "{}",
        outcome.stdout
    );
    outcome.assert_golden("clone-floor/rust");
}

#[test]
fn at_a_floor_of_20_the_setters_pair_as_well() {
    let dir = fixture(Some("[checks.\"static.clones\"]\nmin_nodes = 20\n"));
    let outcome = clones(dir.path());
    assert_eq!(
        groups(&outcome),
        [
            pair("Results::with_reuse", "Plan::with_limit", 22),
            pair("OracleKind::as_str", "Phase::as_str", 35),
        ],
        "{}",
        outcome.stdout
    );
}
