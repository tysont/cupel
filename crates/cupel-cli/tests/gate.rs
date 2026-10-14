// ABOUTME: Integration tests for gates on `cupel check` (ADR-060): the default and `--gate`/`--no-gate`,
// ABOUTME: which table a scope uses, suppressed findings, and the rerun action for an unevaluated rule.
mod support;

use std::process::Command;

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
    assert_eq!(
        envelope["checks"][0]["metrics"]["params_max"],
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
    let failed = repo.run(&["check"]);
    assert_eq!(failed.code, 1, "{}", failed.stdout);
    let envelope = failed.envelope();
    assert_eq!(gate(&envelope)["rules"][0]["actual"], json!(1.0));
    let id = envelope["findings"][0]["id"].as_str().unwrap().to_owned();
    let suppressed = repo.run(&["suppress", &id, "--reason", "accepted"]);
    assert_eq!(suppressed.code, 0, "{}", suppressed.stdout);
    let passed = repo.run(&["check"]);
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

fn git(repo: &Repo, args: &[&str]) -> bool {
    Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(repo.root())
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn a_changed_scope_uses_gate_changed_and_an_unchanged_one_passes_it() {
    let repo = repo(
        "[gate.changed]\n\"static.complexity.params_max\" = \"< 5\"\n\
         [gate.tree]\n\"static.complexity.params_max\" = \"< 10\"\n",
    );
    repo.write(".gitignore", ".cupel/\n");
    if !git(&repo, &["init", "--quiet", "--initial-branch", "main"]) {
        eprintln!("SKIPPED: git not installed");
        return;
    }
    assert!(git(&repo, &["add", "--all"]));
    assert!(git(&repo, &["commit", "--quiet", "-m", "first"]));
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
