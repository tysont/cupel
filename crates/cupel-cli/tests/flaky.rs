// ABOUTME: `tests.flaky` through the binary on the Python tests.flaky fixture (ADR-078): flaky versus
// ABOUTME: order-dependent findings, the text line, `explain`, the reproducing seed, and exit 4 on a red baseline.
mod support;

use serde_json::{Value, json};
use support::Repo;

/// A copy of the Python tests.flaky fixture; `None` (reported as skipped) without pytest.
fn fixture() -> Option<Repo> {
    if cupel_core::find_program("pytest").is_none() {
        eprintln!("SKIPPED: pytest not installed; tests.flaky was not run");
        return None;
    }
    let repo = Repo::new();
    cupel_fixtures::copy_repo("tests.flaky/python", &repo.root());
    Some(repo)
}

const SEED: &str = "3";

/// Each finding's kind and test, in envelope order.
fn found(envelope: &Value) -> Vec<(String, String)> {
    envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            (
                finding["kind"].as_str().unwrap().to_owned(),
                finding["evidence"]["test"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn pair(kind: &str, test: &str) -> (String, String) {
    (kind.to_owned(), test.to_owned())
}

#[test]
fn the_order_dependent_test_and_the_flaky_test_are_told_apart() {
    let Some(repo) = fixture() else { return };
    let outcome = repo.run(&["check", "--only", "tests.flaky", "--seed", SEED]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(
        found(&envelope),
        [
            pair(
                "test.order-dependent",
                "tests/test_ledger.py::LedgerTest::test_balance_starts_at_zero"
            ),
            pair(
                "test.flaky",
                "tests/test_settle.py::SettleTest::test_settles_on_every_other_run"
            ),
        ]
    );
    assert_eq!(
        envelope["findings"][0]["repro"],
        json!(format!("cupel check --only tests.flaky --seed {SEED}"))
    );
    assert_eq!(envelope["run"]["seed"], json!(3));
}

#[test]
fn text_names_each_unstable_test_and_explain_shows_its_source() {
    let Some(repo) = fixture() else { return };
    let text = repo.run(&[
        "check",
        "--only",
        "tests.flaky",
        "--seed",
        SEED,
        "--format",
        "text",
    ]);
    assert_eq!(text.code, 0, "{}", text.stdout);
    assert!(
        text.stdout
            .contains("`test_balance_starts_at_zero` passes or fails depending on the test order"),
        "{}",
        text.stdout
    );
    assert!(
        text.stdout.contains(
            "`test_settles_on_every_other_run` passed and failed in reruns of the same order"
        ),
        "{}",
        text.stdout
    );

    let envelope = repo
        .run(&["check", "--only", "tests.flaky", "--seed", SEED])
        .envelope();
    let id = envelope["findings"][0]["id"].as_str().unwrap().to_owned();
    let explained = repo.run(&["explain", &id]);
    assert_eq!(explained.code, 0, "{}", explained.stdout);
    let explanation = &explained.envelope()["explanation"];
    assert_eq!(
        explanation["excerpt"]["file"],
        json!("tests/test_ledger.py")
    );
    assert!(
        explanation["excerpt"]["lines"]
            .as_array()
            .unwrap()
            .iter()
            .any(|line| line["text"] == json!("    def test_balance_starts_at_zero(self):")),
        "{explanation}"
    );
}

#[test]
fn a_red_baseline_refuses_the_reruns_with_exit_4() {
    let Some(repo) = fixture() else { return };
    repo.write(
        "tests/test_red.py",
        "def test_reports_a_wrong_total():\n    assert 2 + 2 == 5\n",
    );
    let outcome = repo.run(&["check", "--only", "tests.flaky", "--seed", SEED]);
    assert_eq!(outcome.code, 4, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["checks"], json!([]));
    let kinds: Vec<&str> = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["baseline.test-failed"]);
}

#[test]
fn a_repository_without_a_test_suite_skips_tests_flaky_as_unavailable() {
    let repo = Repo::new();
    repo.write(
        "calc.rs",
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    );
    repo.write("calc.py", "def add(a, b):\n    return a + b\n");
    let outcome = repo.run(&["check", "--only", "tests.flaky", "--seed", SEED]);
    let envelope = outcome.envelope();
    assert_eq!(
        (outcome.code, &envelope["status"], &envelope["findings"]),
        (0, &json!("pass"), &json!([])),
        "{envelope}"
    );
    assert_eq!(
        envelope["checks"][0]["skipped"],
        json!({
            "reason": "unavailable",
            "fact": "test-results",
            "lang": null,
            "tool": null,
            "see": "cupel describe matrix"
        })
    );
}
