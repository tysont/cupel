// ABOUTME: Integration tests for gates on `cupel check` (ADR-060): the default and `--gate`/`--no-gate`,
// ABOUTME: which table a scope uses, suppressed findings, rerun actions, and rules on every kind of check.
mod support;

use serde_json::{Value, json};
use support::Repo;

/// One function of cyclomatic complexity 3 and seven parameters (one `params.many` finding).
const CODE: &str = "pub fn wide(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) -> i32 {\n    if a > 1 { b } else if c > 2 { d } else { e + f + g }\n}\n";

fn repo(config: &str) -> Repo {
    let repo = Repo::new();
    repo.write("src/wide.rs", CODE);
    repo.write("cupel.toml", config);
    repo
}

fn gate(envelope: &Value) -> &Value {
    &envelope["summary"]["gate"]
}

#[test]
fn the_gate_is_evaluated_by_default_when_its_table_exists() {
    let repo = repo("[gate.tree]\n\"static.complexity.cyclomatic_max\" = \"<= 3\"\n");
    let outcome = repo.run(&["check"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("pass"));
    assert_eq!(gate(&envelope)["result"], json!("pass"));
    assert_eq!(gate(&envelope)["rules"][0]["actual"], json!(3.0));
    let complexity = envelope["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == json!("static.complexity"))
        .unwrap();
    assert_eq!(
        complexity["metrics"]["params_max"],
        json!(7.0),
        "every scope metric is in the check's result"
    );
}

#[test]
fn no_gate_skips_a_failing_gate() {
    let repo = repo("[gate.tree]\n\"static.complexity.params_max\" = \"< 5\"\n");
    assert_eq!(repo.run(&["check"]).code, 1);
    let outcome = repo.run(&["check", "--no-gate"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(*gate(&outcome.envelope()), Value::Null);
}

#[test]
fn explicit_gate_without_the_applicable_table_is_a_usage_error_before_any_check_runs() {
    let repo = repo("[gate.changed]\n\"static.complexity.params_max\" = \"< 5\"\n");
    let outcome = repo.run(&["check", "--gate"]);
    assert_eq!(outcome.code, 2, "{}", outcome.stdout);
    let finding = outcome.finding();
    assert_eq!(finding["evidence"]["argument"], json!("--gate"));
    assert!(
        finding["title"].as_str().unwrap().contains("[gate.tree]"),
        "{finding}"
    );
    assert_eq!(outcome.envelope()["checks"], json!([]));
    let without = repo.run(&["check"]);
    assert_eq!(without.code, 0, "a tree scope has no [gate.tree] to apply");
    assert_eq!(*gate(&without.envelope()), Value::Null);
}

#[test]
fn a_none_rule_ignores_suppressed_findings() {
    let repo = repo("[gate.tree]\n\"static.complexity\" = \"none\"\n");
    let failed = repo.run(&["check", "--only", "static.complexity"]);
    assert_eq!(failed.code, 1, "{}", failed.stdout);
    let envelope = failed.envelope();
    assert_eq!(gate(&envelope)["rules"][0]["actual"], json!(1.0));
    let id = envelope["findings"][0]["id"].as_str().unwrap().to_owned();
    let suppressed = repo.run(&["suppress", &id, "--reason", "accepted"]);
    assert_eq!(suppressed.code, 0, "{}", suppressed.stdout);
    let passed = repo.run(&["check", "--only", "static.complexity"]);
    assert_eq!(passed.code, 0, "{}", passed.stdout);
    let envelope = passed.envelope();
    assert_eq!(gate(&envelope)["rules"][0]["actual"], json!(0.0));
    assert_eq!(
        envelope["next_actions"],
        json!([]),
        "a suppressed finding needs no action"
    );
}

#[test]
fn an_unselected_check_fails_the_gate_with_a_rerun_action() {
    let repo = repo("[gate.tree]\n\"static.complexity.cyclomatic_max\" = \"< 30\"\n");
    let outcome = repo.run(&["check", "--skip", "static.complexity", "--paths", "src"]);
    assert_eq!(outcome.code, 1, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(
        gate(&envelope)["rules"][0],
        json!({"rule": "static.complexity.cyclomatic_max < 30", "actual": null,
               "result": "skipped"})
    );
    assert_eq!(
        envelope["next_actions"][0],
        json!({
            "priority": 1, "finding": null, "kind": "rerun",
            "summary": "Run `static.complexity`: the gate rule \
                        `static.complexity.cyclomatic_max < 30` was not evaluated in this run.",
            "command": "cupel check --paths src --only static.complexity"
        })
    );
}

#[test]
fn a_changed_scope_uses_gate_changed_and_an_unchanged_one_passes_it() {
    let repo = repo(
        "[gate.changed]\n\"static.complexity.params_max\" = \"< 5\"\n\
         [gate.tree]\n\"static.complexity.params_max\" = \"< 10\"\n",
    );
    repo.write(".gitignore", ".cupel/\n");
    if !repo.committed("first") {
        eprintln!("SKIPPED: git not installed");
        return;
    }
    let unchanged = repo.run(&["check", "--changed"]);
    assert_eq!(unchanged.code, 0, "{}", unchanged.stdout);
    let envelope = unchanged.envelope();
    assert_eq!(gate(&envelope)["evaluated"], json!("gate.changed"));
    assert_eq!(gate(&envelope)["rules"][0]["result"], json!("pass"));

    repo.write("src/wide.rs", &CODE.replace("wide", "wider"));
    let changed = repo.run(&["check", "--changed"]);
    assert_eq!(changed.code, 1, "{}", changed.stdout);
    assert_eq!(gate(&changed.envelope())["rules"][0]["actual"], json!(7.0));

    let tree = repo.run(&["check"]);
    assert_eq!(tree.code, 0, "{}", tree.stdout);
    assert_eq!(gate(&tree.envelope())["evaluated"], json!("gate.tree"));
}

// ---------------------------------------------------------------- rules across checks

/// `risky` has cyclomatic complexity 6: CRAP 42 at coverage 0.
const RISKY: &str = "def risky(a):\n    if a == 1:\n        return 1\n    if a == 2:\n        return 2\n    if a == 3:\n        return 3\n    if a == 4:\n        return 4\n    if a == 5:\n        return 5\n    return 0\n";

/// A Python project with `config` as its `cupel.toml`.
fn python(config: &str) -> Repo {
    let repo = Repo::new();
    repo.write(
        "pyproject.toml",
        "[project]\nname = \"pkg\"\nversion = \"0.1.0\"\n",
    );
    repo.write("cupel.toml", config);
    repo.write("src/pkg/__init__.py", "");
    repo
}

/// Each rule as (rule, actual, result).
fn rules(envelope: &Value) -> Vec<(String, Value, String)> {
    gate(envelope)["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|rule| {
            (
                rule["rule"].as_str().unwrap().to_owned(),
                rule["actual"].clone(),
                rule["result"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn rule(text: &str, actual: Value, result: &str) -> (String, Value, String) {
    (text.to_owned(), actual, result.to_owned())
}

#[test]
fn a_cycle_rule_fails_on_the_tree_and_passes_once_it_is_fixed() {
    let repo = python("[gate.tree]\n\"static.deps.cycles\" = \"== 0\"\n");
    repo.write(
        "src/pkg/alpha.py",
        "from .beta import BETA\n\nALPHA = BETA + 1\n",
    );
    repo.write("src/pkg/beta.py", "from . import alpha\n\nBETA = 2\n");
    let failed = repo.run(&["check"]);
    assert_eq!(failed.code, 1, "{}", failed.stdout);
    let envelope = failed.envelope();
    assert_eq!(envelope["status"], json!("fail"));
    assert_eq!(
        rules(&envelope),
        [rule("static.deps.cycles == 0", json!(1.0), "fail")]
    );
    repo.write("src/pkg/beta.py", "BETA = 2\n");
    let passed = repo.run(&["check"]);
    assert_eq!(passed.code, 0, "{}", passed.stdout);
    assert_eq!(
        rules(&passed.envelope()),
        [rule("static.deps.cycles == 0", json!(0.0), "pass")]
    );
}

#[test]
fn a_crap_rule_cannot_pass_without_coverage_because_crap_is_not_measured() {
    let repo = python("[gate.tree]\n\"static.crap.max\" = \"< 30\"\n");
    repo.write("src/pkg/risky.py", RISKY);
    let outcome = repo.run(&["check"]);
    assert_eq!(outcome.code, 1, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(
        rules(&envelope),
        [rule("static.crap.max < 30", Value::Null, "skipped")]
    );
    let crap = envelope["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == json!("static.crap"))
        .unwrap();
    assert_eq!(crap["status"], json!("pass"));
    assert_eq!(crap["score"], Value::Null);
    assert_eq!(crap["metrics"], json!({}));
}

#[test]
fn a_weakening_presence_rule_fails_on_a_weakened_change_and_passes_on_a_strengthened_one() {
    let repo = python("[gate.changed]\n\"tests.weakening\" = \"none\"\n");
    repo.write(".gitignore", ".cupel/\n");
    let suite = "def test_adds():\n    assert 1 + 1 == 2\n    assert 2 + 2 == 4\n";
    repo.write("tests/test_math.py", suite);
    if !repo.committed("suite") {
        eprintln!("SKIPPED: git not installed");
        return;
    }
    repo.write(
        "tests/test_math.py",
        "def test_adds():\n    assert 1 + 1 == 2\n",
    );
    let failed = repo.run(&["check", "--changed"]);
    assert_eq!(failed.code, 1, "{}", failed.stdout);
    let envelope = failed.envelope();
    assert_eq!(gate(&envelope)["evaluated"], json!("gate.changed"));
    assert_eq!(
        rules(&envelope),
        [rule("tests.weakening none", json!(1.0), "fail")]
    );
    repo.write(
        "tests/test_math.py",
        &format!("{suite}\n\ndef test_multiplies():\n    assert 3 * 3 == 9\n"),
    );
    let passed = repo.run(&["check", "--changed"]);
    assert_eq!(passed.code, 0, "{}", passed.stdout);
    assert_eq!(
        rules(&passed.envelope()),
        [rule("tests.weakening none", json!(0.0), "pass")]
    );
}

#[test]
fn a_tree_gate_rule_on_a_check_that_needs_a_range_is_rejected_at_load() {
    let repo = python("[gate.tree]\n\"tests.weakening\" = \"none\"\n");
    let outcome = repo.run(&["check"]);
    assert_eq!(outcome.code, 2, "{}", outcome.stdout);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    let title = finding["title"].as_str().unwrap();
    assert!(title.contains("`tests.weakening`"), "{title}");
    assert!(
        finding["action"]["summary"]
            .as_str()
            .unwrap()
            .contains("[gate.changed]"),
        "{finding}"
    );
    assert_eq!(outcome.envelope()["checks"], json!([]));
}
