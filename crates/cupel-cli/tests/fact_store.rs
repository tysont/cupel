// ABOUTME: Integration tests for the persistence hook: every run that keeps a run record is also
// ABOUTME: recorded in `.cupel/cupel.db`; dry runs and `describe` are not; failures only reach stderr.
mod support;

use std::fs;

use serde_json::json;
use support::Repo;

#[test]
fn a_run_is_recorded_in_the_store_equal_to_its_run_record() {
    let repo = Repo::new();
    let outcome = repo.run(&["doctor"]);
    let envelope = outcome.envelope();
    let id = envelope["run"]["id"].as_str().unwrap();
    let stored = repo.store().run(&id.parse().unwrap()).unwrap().unwrap();
    assert_eq!(serde_json::to_value(stored).unwrap(), envelope);
}

#[test]
fn error_envelopes_are_recorded_too() {
    let repo = Repo::new();
    let envelope = repo.run(&["check"]).envelope();
    let id = envelope["run"]["id"].as_str().unwrap();
    let stored = repo.store().run(&id.parse().unwrap()).unwrap();
    assert_eq!(
        stored.map(|run| run.run.id.to_string()),
        Some(id.to_owned())
    );
}

#[test]
fn describe_and_dry_runs_create_no_store() {
    let repo = Repo::new();
    let created = [
        &["describe"][..],
        &["doctor", "--dry-run"],
        &["init"],
        &["baseline", "show", "--dry-run"],
    ]
    .map(|args| {
        repo.run(args);
        repo.path(".cupel/cupel.db").exists()
    });
    assert_eq!(created, [false; 4]);
}

#[test]
fn the_store_and_its_lock_live_under_cupel_and_nothing_else_changes() {
    let repo = Repo::new();
    repo.write("src/main.rs", "fn main() {}\n");
    let before = repo.tree_outside_cupel();
    repo.run(&["doctor"]);
    repo.run(&["doctor"]);
    assert_eq!(repo.tree_outside_cupel(), before);
    let mut cupel: Vec<String> = fs::read_dir(repo.path(".cupel"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    cupel.sort();
    assert_eq!(cupel, [".gitignore", "cupel.db", "cupel.lock", "runs"]);
}

#[test]
fn a_store_that_cannot_be_written_is_reported_on_stderr_and_changes_nothing_else() {
    let repo = Repo::new();
    fs::create_dir_all(repo.path(".cupel/cupel.db")).unwrap();
    let outcome = repo.run(&["doctor", "--quiet"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(outcome.envelope()["status"], json!("pass"));
    assert!(
        outcome
            .stderr
            .starts_with("cupel: could not record the run in the fact store: "),
        "{}",
        outcome.stderr
    );
}
