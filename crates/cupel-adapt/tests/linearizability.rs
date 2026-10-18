// ABOUTME: `tool.porcupine` (ADR-119) on the real checker: the conc.linearizability fixture's histories conform to the
// ABOUTME: `histories` inventory, the history command gets its directory and seed, and a failed command is recorded.
mod support;

use std::fs;
use std::path::Path;

use cupel_core::{
    Availability, Cancellation, FactKind, Histories, HistoryStatus, LangId, RunStatus, Scope,
};
use cupel_fixtures::conformance::{LanguageInventory, check_language, fact_inventories};
use serde_json::{Value, json};

/// What `tool.porcupine` makes of the repository at `root` for Go with `command` as its history
/// command; `None` (reported as skipped) when `go` is not installed.
fn checked(root: &Path, command: &str) -> Option<Histories> {
    let producer = support::producer(
        support::shared_registry(),
        FactKind::Histories,
        "tool.porcupine",
        "go",
    );
    let mut ctx = support::produce_ctx(root, "go", Cancellation::new());
    ctx.seed = 3;
    let table = json!({"history_command": command});
    ctx.config = table.as_object().cloned().unwrap();
    ctx.ctx.lang.insert(LangId::from("go"), ctx.config.clone());
    if let Availability::MissingTool { name, .. } = producer.availability(&ctx.ctx) {
        eprintln!("SKIPPED: {name} not installed; tool.porcupine was not run");
        return None;
    }
    let fact = producer
        .produce(&ctx, &Scope::default(), &mut Vec::new())
        .unwrap();
    Some(fact.downcast::<Histories>().unwrap())
}

/// The fixture's history command, from its `cupel.toml`.
fn fixture_command(root: &Path) -> String {
    let config: toml::Table = fs::read_to_string(root.join("cupel.toml"))
        .unwrap()
        .parse()
        .unwrap();
    config["lang"]["go"]["history_command"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn porcupine_decides_every_construct_of_the_histories_inventory() {
    let fixture = support::own_fixture("conc.linearizability", "go");
    let root = fixture.path();
    let before = support::tree(root);
    let Some(histories) = checked(root, &fixture_command(root)) else {
        return;
    };
    assert_eq!(
        support::tree(root),
        before,
        "tool.porcupine wrote in the tree"
    );
    assert_eq!(histories.run.status, RunStatus::Ran, "{:?}", histories.run);
    let inventory = LanguageInventory::parse(
        &LangId::from("go"),
        &fs::read_to_string(root.join("inventory.toml")).unwrap(),
    )
    .unwrap();
    let fact = serde_json::to_value(&histories).unwrap();
    let failures = check_language(
        &fact_inventories(),
        &inventory,
        &[FactKind::Histories],
        |_| Ok(fact.clone()),
    );
    assert!(failures.is_empty(), "{failures:#?}");
    let statuses: Vec<(&str, HistoryStatus)> = histories
        .histories
        .iter()
        .map(|history| (history.name.as_str(), history.status))
        .collect();
    assert_eq!(statuses.len(), 9, "{statuses:?}");
}

#[test]
fn a_stale_read_is_excerpted_as_the_writes_before_it_and_the_read_that_cannot_follow() {
    let fixture = support::own_fixture("conc.linearizability", "go");
    let root = fixture.path();
    let Some(histories) = checked(root, &fixture_command(root)) else {
        return;
    };
    let stale = histories
        .histories
        .iter()
        .find(|history| history.name == "histories/stale-read.jsonl")
        .unwrap();
    let op = |process: u64, f: &str, value: Value, line: u32| json!({"process": process, "f": f, "key": null, "value": value, "outcome": "ok", "line": line});
    assert_eq!(
        serde_json::to_value(&stale.violations).unwrap(),
        json!([{
            "key": null,
            "anomaly": "non-linearizable",
            "states": [2],
            "linearized": [op(0, "write", json!(1), 2), op(0, "write", json!(2), 4)],
            "stuck": [op(1, "read", json!(1), 6)],
        }])
    );
    assert_eq!(stale.operations, 3);
}

#[test]
fn the_history_command_gets_its_directory_and_the_seed_and_its_failure_is_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let command = "printf '%s\\n' '{\"model\": \"register\"}' \
                   \"{\\\"process\\\": 0, \\\"type\\\": \\\"invoke\\\", \\\"f\\\": \\\"write\\\", \\\"value\\\": $CUPEL_SEED}\" \
                   > \"$CUPEL_HISTORY_DIR/seeded.jsonl\"; echo recorded; exit 3";
    let Some(histories) = checked(dir.path(), command) else {
        return;
    };
    assert_eq!(histories.seed, 3);
    assert_eq!(histories.run.status, RunStatus::Failed);
    assert_eq!(histories.run.detail.as_deref(), Some("recorded"));
    assert_eq!(histories.histories.len(), 1);
    let seeded = &histories.histories[0];
    assert_eq!(
        (seeded.name.as_str(), seeded.status, seeded.operations),
        ("seeded.jsonl", HistoryStatus::Linearizable, 1)
    );
}
