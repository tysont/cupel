// ABOUTME: `tests.mutation` through the binary on the Python tests.mutation fixture (ADR-080): a second run reuses
// ABOUTME: every outcome from the store, a red baseline refuses the run with exit 4, and the survivor's text line.
mod support;

use serde_json::{Value, json};
use support::Repo;

/// A copy of the Python tests.mutation fixture; `None` (reported as skipped) without pytest.
fn fixture() -> Option<Repo> {
    Repo::python_fixture("tests.mutation")
}

/// `check --only tests.mutation`, which must exit 0; its envelope.
fn check(repo: &Repo) -> Value {
    let outcome = repo.run(&["check", "--only", "tests.mutation"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    outcome.envelope()
}

/// The result's metric `name`.
fn metric(envelope: &Value, name: &str) -> Value {
    envelope["checks"][0]["metrics"][name].clone()
}

#[test]
fn a_second_run_with_no_changes_runs_no_mutant_and_reports_the_same() {
    let Some(repo) = fixture() else { return };
    let first = check(&repo);
    assert_eq!(
        (metric(&first, "executed"), metric(&first, "reused")),
        (json!(5.0), json!(0.0))
    );
    let second = check(&repo);
    assert_eq!(
        (metric(&second, "executed"), metric(&second, "reused")),
        (json!(0.0), json!(5.0))
    );
    assert_eq!(second["findings"], first["findings"]);
    assert_eq!(second["checks"][0]["score"], first["checks"][0]["score"]);
}

#[test]
fn a_changed_function_reruns_its_own_mutants_only() {
    let Some(repo) = fixture() else { return };
    check(&repo);
    let source = std::fs::read_to_string(repo.root().join("discount.py")).unwrap();
    repo.write("discount.py", &source.replace("total - 5", "total - 6"));
    let again = check(&repo);
    assert_eq!(
        (metric(&again, "executed"), metric(&again, "reused")),
        (json!(0.0), json!(5.0)),
        "refund is uncovered, so changing it runs nothing"
    );
    repo.write(
        "discount.py",
        &source.replace("return total * 1", "return total * 1  # as charged"),
    );
    let changed = repo.run(&["check", "--only", "tests.mutation"]);
    let envelope = changed.envelope();
    assert_eq!(
        (metric(&envelope, "executed"), metric(&envelope, "reused")),
        (json!(5.0), json!(0.0)),
        "{}",
        changed.stdout
    );
}

#[test]
fn a_red_baseline_refuses_the_run_with_exit_4_and_runs_no_mutant() {
    let Some(repo) = fixture() else { return };
    repo.write(
        "tests/test_red.py",
        "def test_reports_a_wrong_total():\n    assert 2 + 2 == 5\n",
    );
    let outcome = repo.run(&["check", "--only", "tests.mutation"]);
    assert_eq!(outcome.code, 4, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(
        (&envelope["status"], &envelope["checks"]),
        (&json!("error"), &json!([]))
    );
    let kinds: Vec<&str> = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["baseline.test-failed"]);
    let writer = cupel_core::Writer::new(&repo.root(), false).unwrap();
    let store = cupel_store::Store::open_existing(&writer).unwrap().unwrap();
    assert!(store.last_mutant_outcomes().unwrap().is_empty());
}

#[test]
fn text_names_the_surviving_mutant_at_its_site() {
    let Some(repo) = fixture() else { return };
    let text = repo.run(&["check", "--only", "tests.mutation", "--format", "text"]);
    assert_eq!(text.code, 0, "{}", text.stdout);
    assert!(
        text.stdout
            .contains("Mutant survived: `>=` → `>` at discount.py:6"),
        "{}",
        text.stdout
    );
    assert!(
        text.stdout
            .contains("at discount.py:6:14-6:16 in discount.discount"),
        "{}",
        text.stdout
    );
}

/// Writes slots for the repository at `owner` under `root`, last used `ago`.
fn slots_of(root: &std::path::Path, name: &str, owner: &str, ago: std::time::Duration) {
    let dir = root.join(name);
    std::fs::create_dir_all(dir.join("python/0")).unwrap();
    std::fs::write(dir.join("python/0/build.out"), "built\n").unwrap();
    std::fs::write(dir.join("lock"), "").unwrap();
    std::fs::write(dir.join("repository"), owner).unwrap();
    std::fs::File::options()
        .write(true)
        .open(dir.join("repository"))
        .unwrap()
        .set_modified(std::time::SystemTime::now() - ago)
        .unwrap();
}

/// A mutation run claims its slots: it removes those of a repository that is gone and those
/// unused for more than seven days, keeps those used recently, and records its own repository,
/// lock, and size (ADR-141 §3).
#[test]
fn a_mutation_run_removes_the_slots_of_gone_and_idle_repositories_and_records_its_own() {
    let Some(repo) = fixture() else { return };
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("cupel-mutate");
    let (idle, recent) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let day = std::time::Duration::from_secs(24 * 60 * 60);
    slots_of(&root, "gone", "/no/such/repository", day);
    slots_of(&root, "idle", &idle.path().display().to_string(), 8 * day);
    slots_of(&root, "recent", &recent.path().display().to_string(), day);
    let outcome = repo.run_with(
        support::cupel().env("TMPDIR", tmp.path()),
        &["check", "--only", "tests.mutation"],
    );
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let mut left: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left.len(), 2, "{left:?}");
    assert!(left.contains(&"recent".to_owned()), "{left:?}");
    let own = root.join(left.iter().find(|name| *name != "recent").unwrap());
    assert_eq!(
        std::fs::read_to_string(own.join("repository")).unwrap(),
        repo.root().display().to_string()
    );
    assert!(own.join("lock").exists());
    let size: u64 = std::fs::read_to_string(own.join("size"))
        .unwrap()
        .parse()
        .unwrap();
    assert!(size > 0);
}
