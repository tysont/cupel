// ABOUTME: Integration tests for `cupel survey` (ADR-062): rankings over the same scope as `check`, no
// ABOUTME: gate even when one is configured, `--gate` refused, dry runs, and the text rendering.
mod support;

use serde_json::{Value, json};
use support::Repo;

const CODE: &str = "pub fn branchy(a: i32) -> i32 {\n    if a > 1 { 1 } else if a > 2 { 2 } else { 3 }\n}\n\npub fn plain(a: i32) -> i32 {\n    a\n}\n";

fn repo() -> Repo {
    let repo = Repo::new();
    repo.write("src/lib.rs", CODE);
    repo.write("web/index.js", "export function one(a) {\n  return a;\n}\n");
    repo.write(
        "cupel.toml",
        // Quick: the repository has no test suite for `tests.flaky`'s baseline to run.
        "depth = \"quick\"\n[gate.tree]\n\"static.complexity.cyclomatic_max\" = \"< 2\"\n",
    );
    repo
}

fn dimension<'a>(envelope: &'a Value, metric: &str) -> &'a Value {
    envelope["rankings"]["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["metric"] == json!(metric))
        .unwrap_or_else(|| panic!("no {metric} dimension"))
}

#[test]
fn survey_ranks_every_symbol_in_scope_and_never_gates() {
    let repo = repo();
    let outcome = repo.run(&["survey"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["run"]["verb"], json!("survey"));
    assert_eq!(envelope["status"], json!("pass"));
    assert_eq!(envelope["summary"]["gate"], Value::Null);
    assert_eq!(envelope["rankings"]["top"], json!(10));
    let cyclomatic = dimension(&envelope, "cyclomatic");
    assert_eq!(cyclomatic["check"], json!("static.complexity"));
    assert_eq!(cyclomatic["per"], json!("symbol"));
    assert_eq!(cyclomatic["measured"], json!(3));
    let ranked: Vec<(u64, f64, &str, &str)> = cyclomatic["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["rank"].as_u64().unwrap(),
                e["value"].as_f64().unwrap(),
                e["target"]["display"].as_str().unwrap(),
                e["lang"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(ranked[0], (1, 3.0, "branchy", "rust"));
    assert_eq!(ranked.len(), 3);
    assert_eq!(dimension(&envelope, "cognitive")["measured"], json!(3));
    let complexity = envelope["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == json!("static.complexity"))
        .unwrap();
    assert_eq!(complexity["metrics"]["cyclomatic_max"], json!(3.0));
}

#[test]
fn survey_shares_the_scope_flags_of_check() {
    let repo = repo();
    let envelope = repo.run(&["survey", "--paths", "web"]).envelope();
    assert_eq!(envelope["run"]["scope"]["languages"], json!(["javascript"]));
    assert_eq!(dimension(&envelope, "cyclomatic")["measured"], json!(1));
}

#[test]
fn survey_refuses_gate_and_accepts_no_gate() {
    let repo = repo();
    let refused = repo.run(&["survey", "--gate"]);
    assert_eq!(refused.code, 2, "{}", refused.stdout);
    assert_eq!(refused.finding()["evidence"]["argument"], json!("--gate"));
    assert_eq!(repo.run(&["survey", "--no-gate"]).code, 0);
}

#[test]
fn a_survey_dry_run_is_a_plan_without_rankings() {
    let envelope = repo().run(&["survey", "--dry-run"]).envelope();
    assert_eq!(envelope["status"], json!("plan"));
    assert_eq!(envelope["rankings"], Value::Null);
    assert!(
        envelope["plan"]["would_run"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["id"] == json!("static.complexity")),
        "{envelope}"
    );
}

/// Each target as (rank, display, recommend, protection).
fn targets(envelope: &Value) -> Vec<(u64, &str, &str, Value)> {
    envelope["rankings"]["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["rank"].as_u64().unwrap(),
                t["target"]["display"].as_str().unwrap(),
                t["recommend"].as_str().unwrap(),
                t["protection"].clone(),
            )
        })
        .collect()
}

/// `survey --depth standard` on the Python mutation fixture with a cyclomatic limit of 2;
/// `None` without pytest.
fn standard_survey() -> Option<Value> {
    let repo = Repo::python_fixture("tests.mutation")?;
    let config = std::fs::read_to_string(repo.path("cupel.toml")).unwrap();
    repo.write(
        "cupel.toml",
        &format!("{config}\n[checks.\"static.complexity\"]\ncyclomatic_max = 2\n"),
    );
    let outcome = repo.run(&["survey", "--depth", "standard"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    Some(outcome.envelope())
}

/// The kind and summary of the first `n` next actions.
fn first_actions(envelope: &Value, n: usize) -> Vec<(&str, &str)> {
    envelope["next_actions"]
        .as_array()
        .unwrap()
        .iter()
        .take(n)
        .map(|a| (a["kind"].as_str().unwrap(), a["summary"].as_str().unwrap()))
        .collect()
}

#[test]
fn a_standard_survey_protects_the_untested_function_before_simplifying_the_complex_one() {
    let Some(envelope) = standard_survey() else {
        return;
    };
    // `refund` no test runs; `discount` is tested, but one mutant of four survives, and it is
    // more complex than the limit.
    assert_eq!(
        targets(&envelope),
        [
            (
                1,
                "discount.refund",
                "protect",
                json!({"mutation_score": null, "coverage": 0.0, "tests": 0})
            ),
            (
                2,
                "discount.discount",
                "both",
                json!({"mutation_score": 0.75, "coverage": 1.0, "tests": 2})
            ),
        ]
    );
    let score = dimension(&envelope, "mutation_score");
    assert_eq!(
        (
            &score["check"],
            &score["worst"],
            &score["entries"][0]["value"]
        ),
        (&json!("tests.mutation"), &json!("lowest"), &json!(0.75))
    );
    assert_eq!(
        first_actions(&envelope, 3),
        [
            ("write_test", "Write a test that calls `refund`."),
            (
                "write_test",
                "Add a test that fails when `>=` at discount.py:6 becomes `>`."
            ),
            (
                "reduce_complexity",
                "Split `discount` into smaller functions or replace conditional chains with data, \
                 until cyclomatic is at most 2 and cognitive at most 15."
            ),
        ]
    );
}

#[test]
fn text_lists_each_dimension_with_its_ranked_targets() {
    let outcome = repo().run(&["survey", "--format", "text"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let out = &outcome.stdout;
    assert!(out.contains("rankings  top 10 per dimension\n"), "{out}");
    assert!(
        out.contains("  static.complexity.cyclomatic  symbol · 3 measured · "),
        "{out}"
    );
    assert!(
        out.contains("     1. 3  branchy  rust · src/lib.rs:1:1-3:2 · sh_"),
        "{out}"
    );
}
