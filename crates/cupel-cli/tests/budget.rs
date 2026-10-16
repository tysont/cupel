// ABOUTME: `--budget` and `--jobs` through the binary on the Python tests.mutation fixture (ADR-086): a small budget
// ABOUTME: skips mutation and says so, a dry run plans it, a slow run ends at the budget, and `--jobs 1` runs one slot.
mod support;

use std::path::Path;
use std::time::{Duration, Instant};

use cupel_core::budget::{TaskDuration, TaskShape};
use cupel_core::{LangId, ProducerId, RunId};
use serde_json::{Value, json};
use support::{Repo, cupel};

/// A copy of the Python tests.mutation fixture; `None` (reported as skipped) without pytest.
fn fixture() -> Option<Repo> {
    Repo::python_fixture("tests.mutation")
}

/// The entry of `id` in `envelope`'s `checks`.
fn result<'a>(envelope: &'a Value, id: &str) -> &'a Value {
    envelope["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == id)
        .unwrap_or_else(|| panic!("no {id} in {envelope}"))
}

#[test]
fn a_one_second_budget_skips_mutation_and_says_so() {
    let Some(repo) = fixture() else { return };
    let outcome = repo.run(&["check", "--budget", "1s", "--no-gate"]);
    assert_eq!(outcome.code, 5, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], "partial");
    let mutation = result(&envelope, "tests.mutation");
    assert_eq!(mutation["status"], "skipped");
    assert_eq!(mutation["skipped"]["reason"], "budget");
    let est_ms = mutation["skipped"]["est_ms"].as_u64().unwrap();
    assert!(est_ms > 1_000, "{est_ms}");
    // Which cheap checks fit depends on how much of the second starting the process took; each
    // either ran or was left out for the budget.
    for check in envelope["checks"].as_array().unwrap() {
        let skipped = &check["skipped"]["reason"];
        assert!(
            check["status"] != "skipped" || skipped == "budget" || skipped == "needs-range",
            "{check}"
        );
    }
    assert_eq!(envelope["run"]["budget_ms"], json!(1_000));
    let reruns: Vec<&str> = envelope["next_actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["kind"] == "rerun")
        .map(|action| action["command"].as_str().unwrap())
        .collect();
    assert!(
        reruns.contains(&"cupel check --only tests.mutation"),
        "{reruns:?}"
    );
    assert!(
        repo.store().last_mutant_outcomes().unwrap().is_empty(),
        "no mutant ran"
    );
    let text = repo.run(&["check", "--budget", "1s", "--no-gate", "--format", "text"]);
    assert!(
        text.stdout.contains("skipped: budget (lang python, est "),
        "{}",
        text.stdout
    );
}

#[test]
fn a_dry_run_under_a_budget_lists_mutation_in_would_skip() {
    let Some(repo) = fixture() else { return };
    let outcome = repo.run(&["check", "--dry-run", "--budget", "1s"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let plan = outcome.envelope()["plan"].clone();
    let skipped = plan["would_skip"]
        .as_array()
        .unwrap()
        .iter()
        .find(|skip| skip["id"] == "tests.mutation")
        .unwrap_or_else(|| panic!("tests.mutation not in {plan}"))
        .clone();
    assert_eq!(skipped["reason"], "budget");
    assert!(skipped["est_ms"].as_u64().unwrap() > 1_000, "{skipped}");
    let planned: Vec<&str> = plan["would_run"]
        .as_array()
        .unwrap()
        .iter()
        .map(|check| check["id"].as_str().unwrap())
        .collect();
    assert!(!planned.contains(&"tests.mutation"), "{planned:?}");
    let total = plan["est_total_ms"].as_u64().unwrap();
    assert!(total <= 1_000, "{total}");
}

/// Records three runs of `producer` for Python at `ms` each, as if earlier runs had timed them.
fn recorded(repo: &Repo, producers: &[&str], ms: u64) {
    let store = repo.store();
    let python = LangId::from("python");
    for n in 0..3_u8 {
        let durations: Vec<TaskDuration> = producers
            .iter()
            .map(|producer| TaskDuration {
                shape: TaskShape::producer(&ProducerId::from(*producer), &python, false),
                duration_ms: ms,
            })
            .collect();
        let run: RunId = support::run_id(n).parse().unwrap();
        store.record_durations(&run, &durations).unwrap();
    }
}

#[test]
fn a_run_cut_by_its_budget_ends_within_the_budget_and_the_cut_task_s_stop() {
    let Some(repo) = fixture() else { return };
    // The history says the suite takes 50 ms; it now takes two minutes, so the run must cut it.
    recorded(
        &repo,
        &["tool.pytest", "tool.pytest-cov", "engine.mutation"],
        50,
    );
    repo.write(
        "tests/test_slow.py",
        "import time\n\n\ndef test_waits():\n    time.sleep(120)\n",
    );
    // The first start of a freshly built binary can wait on the operating system's checks of it,
    // which is no part of the run.
    assert_eq!(support::run(["describe"]).code, 0);
    let budget = Duration::from_secs(10);
    let started = Instant::now();
    let args = [
        "check",
        "--only",
        "tests.mutation",
        "--budget",
        "10s",
        "--no-gate",
    ];
    let outcome = repo.run(&args);
    let elapsed = started.elapsed();
    assert_eq!(outcome.code, 5, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    // The run's own clock starts with the invocation and ends with its envelope; the cut suite
    // is killed at the deadline, so it may pass the budget only by the time the kill takes.
    let duration = Duration::from_millis(envelope["run"]["duration_ms"].as_u64().unwrap());
    assert!(duration >= budget, "{duration:?}");
    assert!(duration < budget + Duration::from_secs(2), "{duration:?}");
    // The wall clock adds the process's start and exit and the run record; the margin is for a
    // loaded machine. The suite alone would take two minutes.
    assert!(
        elapsed < budget + Duration::from_secs(15),
        "took {elapsed:?}"
    );
    assert_eq!(envelope["status"], "partial");
    let mutation = result(&envelope, "tests.mutation");
    assert_eq!(mutation["skipped"]["reason"], "budget", "{mutation}");
    eprintln!("budget 10 s: the run's clock {duration:?}, wall {elapsed:?}");
}

/// The mutation slots a run with `TMPDIR` at `tmp` left for Python, by name.
fn slots(tmp: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for repo in std::fs::read_dir(tmp.join("cupel-mutate")).unwrap() {
        let python = repo.unwrap().path().join("python");
        for slot in std::fs::read_dir(python).unwrap() {
            found.push(slot.unwrap().file_name().to_string_lossy().into_owned());
        }
    }
    found.sort();
    found
}

/// Runs `check --only tests.mutation` with three mutation slots configured, `extra` arguments,
/// and `TMPDIR` at a fresh directory; the slots it used.
fn mutation_slots(extra: &[&str]) -> Option<Vec<String>> {
    let repo = fixture()?;
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("cupel.toml");
    let fixture_config = std::fs::read_to_string(repo.path("cupel.toml")).unwrap();
    std::fs::write(
        &config,
        format!("{fixture_config}mutation = {{ jobs = 3 }}\n"),
    )
    .unwrap();
    let config = config.display().to_string();
    let mut args = vec![
        "check",
        "--only",
        "tests.mutation",
        "--config",
        config.as_str(),
    ];
    args.extend(extra);
    let outcome = repo.run_with(cupel().env("TMPDIR", tmp.path()), &args);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(
        result(&envelope, "tests.mutation")["metrics"]["executed"],
        json!(5.0)
    );
    Some(slots(tmp.path()))
}

#[test]
fn jobs_one_runs_mutants_one_at_a_time() {
    let Some(serial) = mutation_slots(&["--jobs", "1"]) else {
        return;
    };
    assert_eq!(serial, ["0"]);
    let Some(parallel) = mutation_slots(&[]) else {
        return;
    };
    assert_eq!(parallel, ["0", "1", "2"]);
}

#[test]
fn a_run_records_how_long_its_productions_and_checks_took() {
    let Some(repo) = fixture() else { return };
    let outcome = repo.run(&["check", "--only", "tests.mutation"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let recent = repo.store().recent_durations().unwrap();
    let tasks: Vec<(&str, &str)> = recent
        .iter()
        .map(|(shape, _)| (shape.task.as_str(), shape.class.as_str()))
        .collect();
    for expected in [
        ("tool.pytest", "run"),
        ("engine.mutation", "run"),
        ("tests.mutation", "1"),
    ] {
        assert!(tasks.contains(&expected), "{expected:?} in {tasks:?}");
    }
}

#[test]
fn jobs_must_be_a_positive_integer() {
    let outcome = support::run(["check", "--jobs", "0"]);
    assert_eq!(outcome.code, 2, "{}", outcome.stdout);
    assert_eq!(support::usage_argument(&outcome), json!("--jobs"));
}
