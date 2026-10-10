// ABOUTME: Integration tests for the run infrastructure (ADR-020): run records under `.cupel/runs/`,
// ABOUTME: the `--events` stream, `--dry-run` plans that write nothing, `--write`, and `--seed`.
mod support;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use support::{Outcome, cupel, run, run_with};
use tempfile::TempDir;

/// A temporary repository the binary is pointed at with `--cwd`.
struct Repo {
    dir: TempDir,
}

impl Repo {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("temporary directory"),
        }
    }

    fn root(&self) -> PathBuf {
        self.dir.path().canonicalize().unwrap()
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root().join(relative)
    }

    /// Runs cupel with `--cwd` set to this repository.
    fn run(&self, args: &[&str]) -> Outcome {
        let root = self.root().display().to_string();
        let mut all: Vec<&str> = args.to_vec();
        all.extend(["--cwd", root.as_str()]);
        run(all)
    }

    /// Every path under the root with its content, sorted.
    fn tree(&self) -> Vec<(String, Vec<u8>)> {
        fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                let name = path.strip_prefix(root).unwrap().display().to_string();
                if path.is_dir() {
                    out.push((format!("{name}/"), Vec::new()));
                    walk(&path, root, out);
                } else {
                    out.push((name, fs::read(&path).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root(), &self.root(), &mut out);
        out.sort();
        out
    }

    /// The lines of an events file, parsed.
    fn events(&self, relative: &str) -> Vec<Value> {
        lines(&fs::read_to_string(self.path(relative)).expect("events file exists"))
    }
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
    let outcome = repo.run(&["check"]);
    let envelope = outcome.envelope();
    let id = envelope["run"]["id"].as_str().unwrap();
    let record = fs::read_to_string(repo.path(&format!(".cupel/runs/{id}.json"))).unwrap();
    assert!(record.ends_with("}\n"));
    assert_eq!(serde_json::from_str::<Value>(&record).unwrap(), envelope);
}

#[test]
fn the_run_record_is_json_whatever_the_format_and_holds_every_finding() {
    let repo = Repo::new();
    let outcome = repo.run(&["check", "--format", "text", "--limit", "0"]);
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
    repo.run(&["check"]);
    assert_eq!(
        fs::read_to_string(repo.path(".cupel/.gitignore")).unwrap(),
        "*\n!suppressions.toml\n"
    );
}

#[test]
fn describe_writes_no_run_record() {
    let repo = Repo::new();
    for args in [&["describe"][..], &["check", "--describe"][..]] {
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

#[test]
fn a_dry_run_of_a_pending_verb_is_still_not_implemented_and_writes_nothing() {
    let repo = Repo::new();
    let outcome = repo.run(&["check", "--dry-run"]);
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.finding()["kind"], json!("cli.not-implemented"));
    assert!(repo.tree().is_empty(), "{:?}", repo.tree());
}

#[test]
fn a_dry_run_still_validates_its_command() {
    let repo = Repo::new();
    let outcome = repo.run(&["describe", "nonsense", "--dry-run"]);
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.finding()["kind"], json!("cli.usage"));
}

// ---------------------------------------------------------------- events

#[test]
fn a_dry_run_streams_plan_then_run_end() {
    let repo = Repo::new();
    let outcome = repo.run(&["describe", "--dry-run", "--events", ".cupel/events.jsonl"]);
    let envelope = outcome.envelope();
    let events = repo.events(".cupel/events.jsonl");
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
    let repo = Repo::new();
    let outcome = repo.run(&["check", "--events", ".cupel/events.jsonl"]);
    assert_eq!(outcome.code, 2);
    assert_eq!(
        repo.events(".cupel/events.jsonl"),
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
    let repo = Repo::new();
    let outcome = repo.run(&["describe", "--events", "events.jsonl"]);
    assert_eq!(outcome.code, 2);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    assert_eq!(finding["evidence"]["argument"], json!("--events"));
    assert_eq!(finding["evidence"]["value"], json!("events.jsonl"));
    let fix = finding["action"]["summary"].as_str().unwrap();
    assert!(fix.contains("--write"), "{fix}");
    assert!(!repo.path("events.jsonl").exists());

    let outcome = repo.run(&["describe", "--events", "events.jsonl", "--write"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(names(&repo.events("events.jsonl")), ["run.end"]);
}

#[test]
fn an_events_path_escaping_cupel_is_refused() {
    let repo = Repo::new();
    let outcome = repo.run(&["describe", "--events", ".cupel/../events.jsonl"]);
    assert_eq!(outcome.code, 2);
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
    assert_ne!(first, 0);
    assert_ne!(second, 0);
    assert_ne!(first, second);
}

#[test]
fn the_seed_is_in_the_run_record() {
    let repo = Repo::new();
    let envelope = repo.run(&["check", "--seed", "7"]).envelope();
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
