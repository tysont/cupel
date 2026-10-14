// ABOUTME: Integration tests for the run infrastructure (ADR-020): run records under `.cupel/runs/`,
// ABOUTME: the `--events` stream, `--dry-run` plans that write nothing, `--write`, and `--seed`.
mod support;

use std::fs;

use serde_json::{Value, json};
use support::{Repo, cupel, run, run_with};

/// The lines of an events file in `repo`, parsed.
fn events(repo: &Repo, relative: &str) -> Vec<Value> {
    lines(&fs::read_to_string(repo.path(relative)).expect("events file exists"))
}

fn lines(text: &str) -> Vec<Value> {
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

fn names(events: &[Value]) -> Vec<&str> {
    events
        .iter()
        .map(|event| event["ev"].as_str().unwrap())
        .collect()
}

// ---------------------------------------------------------------- run records

#[test]
fn a_run_writes_its_envelope_to_the_run_record_equal_to_stdout() {
    let repo = Repo::new();
    let outcome = repo.run(&["tests"]);
    let envelope = outcome.envelope();
    let id = envelope["run"]["id"].as_str().unwrap();
    let record = fs::read_to_string(repo.path(&format!(".cupel/runs/{id}.json"))).unwrap();
    assert!(record.ends_with("}\n"));
    assert_eq!(serde_json::from_str::<Value>(&record).unwrap(), envelope);
}

#[test]
fn the_run_record_is_json_whatever_the_format_and_holds_every_finding() {
    let repo = Repo::new();
    let outcome = repo.run(&["tests", "--format", "text", "--limit", "0"]);
    assert!(
        outcome.stdout.contains("error (exit 2)"),
        "{}",
        outcome.stdout
    );
    let runs: Vec<_> = fs::read_dir(repo.path(".cupel/runs")).unwrap().collect();
    assert_eq!(runs.len(), 1);
    let record: Value =
        serde_json::from_str(&fs::read_to_string(runs[0].as_ref().unwrap().path()).unwrap())
            .unwrap();
    assert_eq!(record["findings"].as_array().unwrap().len(), 1);
    assert_eq!(record["truncated"], json!(false));
}

#[test]
fn creating_cupel_also_writes_its_gitignore() {
    let repo = Repo::new();
    repo.run(&["tests"]);
    assert_eq!(
        fs::read_to_string(repo.path(".cupel/.gitignore")).unwrap(),
        "*\n!suppressions.toml\n"
    );
}

#[test]
fn describe_writes_no_run_record() {
    let repo = Repo::new();
    for args in [&["describe"][..], &["tests", "--describe"][..]] {
        assert_eq!(repo.run(args).code, 0);
    }
    assert!(repo.tree().is_empty(), "{:?}", repo.tree());
}

// ---------------------------------------------------------------- dry runs

#[test]
fn a_dry_run_is_a_plan_envelope_and_writes_nothing() {
    let repo = Repo::new();
    fs::write(repo.path("cupel.toml"), "depth = \"quick\"\n").unwrap();
    let before = repo.tree();
    let outcome = repo.run(&["describe", "--dry-run"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("plan"));
    assert_eq!(envelope["description"], Value::Null);
    assert_eq!(
        envelope["plan"],
        json!({"would_run": [], "would_skip": [], "would_install": [], "would_write": [],
               "est_total_ms": 0})
    );
    assert_eq!(repo.tree(), before, "a dry run changes nothing");
}

/// `cupel <args>` run in a fresh repository, asserting it exits with `code`.
fn exits(args: &[&str], code: i32) -> (Repo, support::Outcome) {
    let repo = Repo::new();
    let outcome = repo.run(args);
    assert_eq!(outcome.code, code, "{}", outcome.stdout);
    (repo, outcome)
}

#[test]
fn a_dry_run_of_a_pending_verb_is_still_not_implemented_and_writes_nothing() {
    let (repo, outcome) = exits(&["tests", "--dry-run"], 2);
    assert_eq!(outcome.finding()["kind"], json!("cli.not-implemented"));
    assert!(repo.tree().is_empty(), "{:?}", repo.tree());
}

#[test]
fn a_dry_run_still_validates_its_command() {
    let (_repo, outcome) = exits(&["describe", "nonsense", "--dry-run"], 2);
    assert_eq!(outcome.finding()["kind"], json!("cli.usage"));
}

// ---------------------------------------------------------------- events

#[test]
fn a_dry_run_streams_plan_then_run_end() {
    let repo = Repo::new();
    let outcome = repo.run(&["describe", "--dry-run", "--events", ".cupel/events.jsonl"]);
    let envelope = outcome.envelope();
    let events = events(&repo, ".cupel/events.jsonl");
    assert_eq!(names(&events), ["plan", "run.end"]);
    assert_eq!(
        events[0],
        json!({"ev": "plan", "run": envelope["run"]["id"], "checks": 0, "est_ms": 0})
    );
    assert_eq!(
        events[1],
        json!({"ev": "run.end", "status": "plan", "exit_code": 0})
    );
}

#[test]
fn a_run_streams_run_end_with_the_envelope_status_and_exit_code() {
    let (repo, _outcome) = exits(&["tests", "--events", ".cupel/events.jsonl"], 2);
    assert_eq!(
        events(&repo, ".cupel/events.jsonl"),
        [json!({"ev": "run.end", "status": "error", "exit_code": 2})]
    );
}

#[test]
fn events_on_stderr_leave_stdout_a_valid_envelope() {
    let repo = Repo::new();
    let outcome = repo.run(&["describe", "--dry-run", "--events", "stderr"]);
    assert_eq!(outcome.envelope()["status"], json!("plan"));
    assert_eq!(names(&lines(&outcome.stderr)), ["plan", "run.end"]);
}

#[test]
fn an_events_file_outside_cupel_needs_write() {
    let (repo, outcome) = exits(&["describe", "--events", "events.jsonl"], 2);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    assert_eq!(finding["evidence"]["argument"], json!("--events"));
    assert_eq!(finding["evidence"]["value"], json!("events.jsonl"));
    assert_eq!(
        finding["action"]["summary"],
        json!(
            "Pass --write to permit writing the events file there, put it under .cupel/, or use \
             --events stderr."
        )
    );
    assert!(!repo.path("events.jsonl").exists());

    let outcome = repo.run(&["describe", "--events", "events.jsonl", "--write"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(names(&events(&repo, "events.jsonl")), ["run.end"]);
}

#[test]
fn an_events_path_escaping_cupel_is_refused() {
    let (repo, outcome) = exits(&["describe", "--events", ".cupel/../events.jsonl"], 2);
    let finding = outcome.finding();
    assert_eq!(
        (&finding["kind"], &finding["evidence"]["argument"]),
        (&json!("cli.usage"), &json!("--events"))
    );
    assert!(!repo.path("events.jsonl").exists());
}

// ---------------------------------------------------------------- seeds

#[test]
fn an_explicit_seed_is_echoed() {
    assert_eq!(
        run(["describe", "--seed", "7"]).envelope()["run"]["seed"],
        json!(7)
    );
}

#[test]
fn without_a_seed_a_random_non_zero_seed_is_drawn_per_run() {
    let seed = || {
        run(["describe"]).envelope()["run"]["seed"]
            .as_u64()
            .unwrap()
    };
    let (first, second) = (seed(), seed());
    // (first non-zero, second non-zero, the two differ)
    assert_eq!(
        (first != 0, second != 0, first != second),
        (true, true, true),
        "{first}, {second}"
    );
}

#[test]
fn the_seed_is_in_the_run_record() {
    let repo = Repo::new();
    let envelope = repo.run(&["tests", "--seed", "7"]).envelope();
    let id = envelope["run"]["id"].as_str().unwrap();
    let record: Value = serde_json::from_str(
        &fs::read_to_string(repo.path(&format!(".cupel/runs/{id}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(record["run"]["seed"], json!(7));
}

// ---------------------------------------------------------------- timeouts

#[test]
fn an_invalid_timeout_is_a_usage_error_naming_it() {
    let finding = run(["describe", "--timeout", "soon"]).finding();
    assert_eq!(finding["evidence"]["argument"], json!("--timeout"));
}

#[test]
fn a_timeout_that_does_not_expire_changes_nothing() {
    let outcome = run_with(
        cupel().env("CUPEL_DEBUG_SLEEP_MS", "10"),
        ["describe", "--timeout", "1h"],
    );
    assert_eq!(outcome.code, 0);
    assert_eq!(outcome.envelope()["status"], json!("pass"));
}
