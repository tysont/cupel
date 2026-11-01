// ABOUTME: static.clones' size floor and enum mappings end to end (ADR-158): 22-node setters do not pair, 35-node
// ABOUTME: `match` accessors pair at info, and three 30-node `as_str` accessors are an info cluster shaped `enum-mapping`.
use std::path::Path;
use std::process::Command;

use cupel_fixtures::{Outcome, copy_repo, run_cupel};
use serde_json::{Value, json};

/// A git copy of the fixture `repo`, with `config` as its `cupel.toml` when given.
fn fixture(repo: &str, config: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo(repo, dir.path());
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

/// Each group's members as (symbol, nodes): a pair's two, a cluster's every member.
fn groups(outcome: &Outcome) -> Vec<Vec<(String, u64)>> {
    outcome
        .envelope()
        .findings
        .iter()
        .map(|finding| {
            let evidence = &finding.evidence.0;
            let members: Vec<&Value> = match evidence.get("members") {
                Some(Value::Array(members)) => members.iter().collect(),
                _ => vec![&evidence["left"], &evidence["right"]],
            };
            members
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

/// Each finding's severity and evidence shape.
fn shapes(outcome: &Outcome) -> Vec<(String, Value)> {
    outcome
        .envelope()
        .findings
        .iter()
        .map(|finding| {
            let severity = serde_json::to_value(finding.severity).unwrap();
            let shape = finding.evidence.0.get("shape").cloned();
            (
                severity.as_str().unwrap().to_owned(),
                shape.unwrap_or(Value::Null),
            )
        })
        .collect()
}

#[test]
fn a_22_node_setter_pair_falls_below_the_floor_and_a_35_node_match_pair_does_not() {
    let dir = fixture("clone-floor/rust", None);
    let outcome = clones(dir.path());
    assert_eq!(
        groups(&outcome),
        [pair("OracleKind::as_str", "Phase::as_str", 35)],
        "{}",
        outcome.stdout
    );
    assert_eq!(
        shapes(&outcome),
        [("info".to_owned(), json!("enum-mapping"))]
    );
    outcome.assert_golden("clone-floor/rust");
}

#[test]
fn at_a_floor_of_20_the_setters_pair_as_well() {
    let dir = fixture(
        "clone-floor/rust",
        Some("[checks.\"static.clones\"]\nmin_nodes = 20\n"),
    );
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
    assert_eq!(
        shapes(&outcome),
        [
            ("warn".to_owned(), Value::Null),
            ("info".to_owned(), json!("enum-mapping"))
        ]
    );
}

#[test]
fn three_30_node_as_str_accessors_are_a_cluster_at_info_shaped_enum_mapping() {
    let dir = fixture("clone-mapping/rust", None);
    let outcome = clones(dir.path());
    let member = |symbol: &str| (symbol.to_owned(), 30);
    assert_eq!(
        groups(&outcome),
        [vec![
            member("Source::as_str"),
            member("Phase::as_str"),
            member("Reuse::as_str")
        ]],
        "{}",
        outcome.stdout
    );
    assert_eq!(
        shapes(&outcome),
        [("info".to_owned(), json!("enum-mapping"))]
    );
    outcome.assert_golden("clone-mapping/rust");
}
