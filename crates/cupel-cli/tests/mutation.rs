// ABOUTME: `tests.mutation` through the binary on the Python tests.mutation fixture (ADR-080): a second run reuses
// ABOUTME: every outcome from the store, a red baseline refuses the run with exit 4, and the survivor's text line.
mod support;

use serde_json::{Value, json};
use support::Repo;

/// A copy of the Python tests.mutation fixture; `None` (reported as skipped) without pytest.
fn fixture() -> Option<Repo> {
    if cupel_core::find_program("pytest").is_none() {
        eprintln!("SKIPPED: pytest not installed; tests.mutation was not run");
        return None;
    }
    let repo = Repo::new();
    cupel_fixtures::copy_repo("tests.mutation/python", &repo.root());
    Some(repo)
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
