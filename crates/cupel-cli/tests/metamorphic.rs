// ABOUTME: `gen.metamorphic` through the binary on the Python gen.metamorphic fixture (ADR-097, ADR-098): the violated
// ABOUTME: relation's finding and a gate on it, `--cases`, the unconfigured skip, and malformed relations as exit 2.
mod support;

use serde_json::{Value, json};
use support::Repo;

/// A copy of the Python gen.metamorphic fixture; `None` (reported as skipped) without python3.
fn fixture() -> Option<Repo> {
    if cupel_core::find_program("python3").is_none() {
        eprintln!("SKIPPED: python3 not installed; gen.metamorphic was not run");
        return None;
    }
    let repo = Repo::new();
    cupel_fixtures::copy_repo("gen.metamorphic/python", &repo.root());
    Some(repo)
}

/// The check's result in `envelope`.
fn result(envelope: &Value) -> &Value {
    envelope["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == json!("gen.metamorphic"))
        .expect("gen.metamorphic has a result")
}

#[test]
fn the_violated_relation_fails_the_check_with_its_minimized_input() {
    let Some(repo) = fixture() else { return };
    let outcome = repo.run(&["check", "--only", "gen.metamorphic", "--seed", "3"]);
    // Without a gate rule a failing check does not fail the run (exit 1 is the gate's).
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(result(&envelope)["status"], json!("fail"));
    let findings = envelope["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1, "{findings:#?}");
    let finding = &findings[0];
    assert_eq!(
        (
            &finding["kind"],
            &finding["evidence"]["relation"],
            &finding["evidence"]["input"]
        ),
        (
            &json!("metamorphic.violated"),
            &json!("sort-reverse"),
            &json!([0, 0, -1])
        )
    );
    assert_eq!(
        finding["repro"],
        json!("cupel check --only gen.metamorphic --seed 3")
    );
    assert_eq!(
        result(&envelope)["metrics"],
        json!({"relations": 2.0, "violated": 1.0})
    );
    assert_eq!(repo.tree_outside_cupel(), {
        let fresh = Repo::new();
        cupel_fixtures::copy_repo("gen.metamorphic/python", &fresh.root());
        fresh.tree_outside_cupel()
    });
}

#[test]
fn the_case_budget_limits_every_relation_to_that_many_inputs() {
    let Some(repo) = fixture() else { return };
    let outcome = repo.run(&[
        "check",
        "--only",
        "gen.metamorphic",
        "--seed",
        "3",
        "--cases",
        "2",
    ]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["findings"], json!([]));
    assert_eq!(result(&envelope)["metrics"]["violated"], json!(0.0));
}

#[test]
fn without_relations_the_check_is_skipped_as_unconfigured() {
    let repo = Repo::new();
    repo.write("calc.py", "def add(a, b):\n    return a + b\n");
    let outcome = repo.run(&["check", "--only", "gen.metamorphic"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    let skipped = &result(&envelope)["skipped"];
    assert_eq!(
        (&skipped["reason"], &skipped["lang"]),
        (&json!("unconfigured"), &json!("python"))
    );
}

#[test]
fn a_malformed_relation_is_a_usage_error_naming_it() {
    let repo = Repo::new();
    repo.write("calc.py", "def add(a, b):\n    return a + b\n");
    repo.write(
        "cupel.toml",
        "[lang.python.relations.adds]\nfunction = \"calc:add\"\ninput = \"float\"\ntransform = \"abs\"\n",
    );
    let outcome = repo.run(&["check", "--only", "gen.metamorphic"]);
    assert_eq!(outcome.code, 2, "{}", outcome.stdout);
    assert!(
        outcome.stdout.contains(
            "invalid relation `adds` in `[lang.python.relations]`: invalid input type `float`"
        ),
        "{}",
        outcome.stdout
    );
}

#[test]
fn a_gate_on_violated_relations_fails_the_run() {
    let Some(repo) = fixture() else { return };
    let config = std::fs::read_to_string(repo.path("cupel.toml")).unwrap();
    repo.write(
        "cupel.toml",
        &format!("{config}\n[gate.tree]\n\"gen.metamorphic.violated\" = \"== 0\"\n"),
    );
    let outcome = repo.run(&[
        "check",
        "--only",
        "gen.metamorphic",
        "--seed",
        "3",
        "--gate",
    ]);
    assert_eq!(outcome.code, 1, "{}", outcome.stdout);
    assert_eq!(
        outcome.envelope()["summary"]["gate"]["result"],
        json!("fail")
    );
}
