// ABOUTME: Integration tests for `cupel check`: scope flags, selection, events, dry runs, usage errors,
// ABOUTME: configuration validation, and the envelope's checks, findings, scope, and next actions.
mod support;

use std::fs;

use serde_json::{Value, json};
use support::{Repo, finding_fields, usage_argument};

/// A Rust function with seven parameters: one `params.many` finding at the defaults.
const WIDE: &str = "pub fn wide(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) -> i32 {\n    a + b + c + d + e + f + g\n}\n";
const NARROW: &str = "pub fn narrow(a: i32) -> i32 {\n    a\n}\n";

fn repo_with_code() -> Repo {
    let repo = Repo::new();
    repo.write("src/wide.rs", WIDE);
    repo.write("src/narrow.rs", NARROW);
    repo.write("web/index.js", "export function one(a) {\n  return a;\n}\n");
    repo
}

#[test]
fn a_tree_run_reports_findings_checks_scope_and_next_actions() {
    let repo = repo_with_code();
    let outcome = repo.run(&["check", "--only", "static.complexity"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("pass"));
    assert_eq!(finding_fields(&envelope, "kind"), ["params.many"]);
    let scope = &envelope["run"]["scope"];
    assert_eq!(scope["mode"], json!("tree"));
    assert_eq!(scope["files"], json!(3));
    assert_eq!(scope["symbols"], json!(3));
    assert_eq!(scope["languages"], json!(["javascript", "rust"]));
    let check = &envelope["checks"][0];
    assert_eq!(check["id"], json!("static.complexity"));
    assert_eq!(check["status"], json!("fail"));
    assert_eq!(check["findings"], json!(1));
    assert_eq!(check["score"]["n"], json!(3));
    assert_eq!(envelope["summary"]["findings"]["warn"], json!(1));
    assert_eq!(envelope["summary"]["gate"], Value::Null);
    assert_eq!(
        envelope["next_actions"][0]["finding"],
        envelope["findings"][0]["id"]
    );
    assert_eq!(
        envelope["next_actions"][0]["kind"],
        json!("refactor_extract")
    );
}

#[test]
fn scope_languages_are_exactly_the_languages_that_saw_files() {
    let repo = repo_with_code();
    // Quick: these repositories have no test suite for `tests.flaky`'s baseline to run.
    let envelope = repo
        .run(&["check", "--paths", "src", "--depth", "quick"])
        .envelope();
    assert_eq!(envelope["run"]["scope"]["languages"], json!(["rust"]));
    assert_eq!(envelope["run"]["scope"]["mode"], json!("paths"));
    let empty = Repo::new();
    let envelope = empty.run(&["check"]).envelope();
    assert_eq!(envelope["run"]["scope"]["languages"], json!([]));
    assert_eq!(envelope["run"]["scope"]["symbols"], json!(0));
}

#[test]
fn symbols_select_by_name_or_hash_and_repro_commands_reproduce_findings() {
    let repo = repo_with_code();
    let by_name = repo
        .run(&[
            "check",
            "--symbols",
            "wide::wide",
            "--only",
            "static.complexity",
        ])
        .envelope();
    assert_eq!(by_name["run"]["scope"]["symbols"], json!(1));
    assert_eq!(finding_fields(&by_name, "kind"), ["params.many"]);
    let command = by_name["findings"][0]["repro"].as_str().unwrap().to_owned();
    let args: Vec<&str> = command.split_whitespace().skip(1).collect();
    let again = repo.run(&args).envelope();
    assert_eq!(again["findings"][0]["id"], by_name["findings"][0]["id"]);
    assert_eq!(again["run"]["scope"]["mode"], json!("symbols"));
}

#[test]
fn unknown_paths_symbols_checks_and_tiers_are_usage_errors_naming_the_flag() {
    let repo = repo_with_code();
    for (args, flag) in [
        (vec!["check", "--paths", "scr"], "--paths"),
        (vec!["check", "--symbols", "nothing::here"], "--symbols"),
        (vec!["check", "--only", "static.complexty"], "--only"),
        (vec!["check", "--skip", "spec"], "--skip"),
        (vec!["check", "--tier", "9+"], "--tier"),
        (vec!["check", "--changed"], "--changed"),
        (vec!["check", "--base", "main"], "--base"),
    ] {
        assert_eq!(usage_argument(&repo.run(&args)), json!(flag), "{args:?}");
    }
}

#[test]
fn configuration_naming_unknown_checks_metrics_or_settings_is_a_usage_error() {
    for (toml, subject) in [
        (
            "[checks.\"static.complexty\"]\nx = 1\n",
            "`static.complexty`",
        ),
        (
            "[gate.tree]\n\"static.complexity.score\" = \"< 3\"\n",
            "metric `score`",
        ),
        (
            "[checks.\"static.complexity\"]\nparams = 3\n",
            "setting `params`",
        ),
    ] {
        let repo = repo_with_code();
        repo.write("cupel.toml", toml);
        let outcome = repo.run(&["check"]);
        assert_eq!(outcome.code, 2, "{toml}: {}", outcome.stdout);
        let finding = outcome.finding();
        assert_eq!(finding["kind"], json!("cli.usage"), "{toml}");
        let detail = format!("{finding}");
        assert!(detail.contains(subject), "{toml}: {detail}");
    }
    let repo = repo_with_code();
    repo.write(
        "cupel.toml",
        "depth = \"quick\"\n[gate.tree]\n\"static.complexity.cyclomatic_max\" = \"< 30\"\n",
    );
    assert_eq!(
        repo.run(&["check"]).code,
        0,
        "a declared metric is a valid gate key"
    );
}

#[test]
fn thresholds_and_per_language_overrides_come_from_the_config() {
    let repo = repo_with_code();
    repo.write(
        "cupel.toml",
        "[checks.\"static.complexity\"]\nparams_max = 0\n[checks.\"static.complexity\".lang.rust]\nparams_max = 10\n",
    );
    let envelope = repo
        .run(&["check", "--only", "static.complexity"])
        .envelope();
    let files: Vec<&str> = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["location"]["file"].as_str().unwrap())
        .collect();
    assert_eq!(
        files,
        ["web/index.js"],
        "only the JavaScript symbol exceeds 0"
    );
}

#[test]
fn a_run_streams_plan_check_events_findings_and_run_end() {
    let repo = repo_with_code();
    let outcome = repo.run(&[
        "check",
        "--only",
        "static.complexity",
        "--events",
        ".cupel/events.jsonl",
    ]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let text = fs::read_to_string(repo.path(".cupel/events.jsonl")).unwrap();
    let names: Vec<String> = text
        .lines()
        .map(|line| {
            let event: Value = serde_json::from_str(line).unwrap();
            event["ev"].as_str().unwrap().to_owned()
        })
        .collect();
    assert_eq!(
        names,
        [
            "plan",
            "check.start",
            "progress",
            "finding",
            "progress",
            "check.end",
            "run.end"
        ]
    );
}

#[test]
fn a_dry_run_plans_targets_and_the_run_record_and_writes_nothing() {
    let repo = repo_with_code();
    let before = repo.tree();
    let outcome = repo.run(&["check", "--only", "static.complexity", "--dry-run"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("plan"));
    let plan = &envelope["plan"];
    assert_eq!(plan["would_run"][0]["id"], json!("static.complexity"));
    assert_eq!(plan["would_run"][0]["targets"], json!(3));
    // Three quick targets (1 ms each) and the two quick productions planning did not make,
    // `decisions` and `norm-tree` (100 ms each, ADR-086 §3).
    assert_eq!(plan["would_run"][0]["est_ms"], json!(203));
    assert_eq!(plan["est_total_ms"], json!(203));
    assert_eq!(plan["would_skip"], json!([]));
    assert_eq!(
        plan["would_write"],
        json!([{"path": ".cupel/runs/<id>.json", "diff": null}])
    );
    assert_eq!(envelope["run"]["scope"]["symbols"], json!(3));
    assert!(envelope["checks"].as_array().unwrap().is_empty());
    assert_eq!(repo.tree(), before, "a dry run writes nothing");
}

/// Runs `git <args>` in `repo`; false when git is not installed.
#[test]
fn changed_on_a_clean_tree_passes_with_no_symbols_and_then_sees_edits() {
    let repo = repo_with_code();
    repo.write(".gitignore", ".cupel/\n");
    if !repo.git(&["init", "--quiet", "--initial-branch", "main"]) {
        eprintln!("SKIPPED: git not installed");
        return;
    }
    assert!(repo.git(&["add", "--all"]));
    assert!(repo.git(&["commit", "--quiet", "-m", "first"]));
    let clean = repo.run(&["check", "--only", "static.complexity", "--changed"]);
    assert_eq!(clean.code, 0, "{}", clean.stdout);
    let envelope = clean.envelope();
    assert_eq!(envelope["status"], json!("pass"));
    assert_eq!(envelope["run"]["scope"]["mode"], json!("changed"));
    assert_eq!(envelope["run"]["scope"]["symbols"], json!(0));
    assert_eq!(envelope["run"]["git"]["dirty"], json!(false));
    assert!(envelope["run"]["git"]["base"].is_string());

    repo.write("src/narrow.rs", WIDE.replace("wide", "narrow").as_str());
    let edited = repo
        .run(&["check", "--only", "static.complexity", "--changed"])
        .envelope();
    assert_eq!(edited["run"]["scope"]["files"], json!(1));
    assert_eq!(finding_fields(&edited, "kind"), ["params.many"]);
    assert_eq!(edited["run"]["git"]["dirty"], json!(true));
}

#[test]
fn scope_exclude_keeps_nested_projects_out_of_files_roots_languages_and_ranges() {
    let repo = repo_with_code();
    repo.write("Cargo.toml", "[package]\nname = \"x\"\n");
    repo.write("fixtures/repos/go/go.mod", "module x\n");
    repo.write(
        "fixtures/repos/go/main.go",
        "package main\n\nfunc main() {}\n",
    );
    repo.write("fixtures/repos/py/pyproject.toml", "");
    repo.write("fixtures/repos/py/app.py", "def f():\n    return 1\n");
    repo.write(".gitignore", ".cupel/\n");
    repo.write(
        "cupel.toml",
        "[scope]\nexclude = [\"fixtures/**\", \"web\"]\n",
    );
    let tree = repo
        .run(&["check", "--only", "static.complexity"])
        .envelope();
    assert_eq!(tree["run"]["scope"]["files"], json!(2), "{tree:#}");
    assert_eq!(tree["run"]["scope"]["roots"], json!(["."]));
    assert_eq!(tree["run"]["scope"]["languages"], json!(["rust"]));
    if !repo.git(&["init", "--quiet", "--initial-branch", "main"]) {
        eprintln!("SKIPPED: git not installed");
        return;
    }
    assert!(repo.git(&["add", "--all"]));
    assert!(repo.git(&["commit", "--quiet", "-m", "first"]));
    repo.write("fixtures/repos/py/app.py", "def g():\n    return 2\n");
    let changed = repo
        .run(&["check", "--only", "static.complexity", "--changed"])
        .envelope();
    assert_eq!(changed["run"]["scope"]["files"], json!(0), "{changed:#}");
    assert_eq!(changed["run"]["scope"]["languages"], json!([]));
}
