// ABOUTME: The suite joint's incremental run end to end (ADR-149): a commit changes one function of a Rust crate whose
// ABOUTME: suite ran whole once, and the real binary's `--base` run reruns one test, keeps three, and grades the same.
use std::path::Path;
use std::process::Command;

use cupel_core::{Availability, Ctx, FactKind, Lang, LangFamily, LangId, ProducerRegistry};
use cupel_fixtures::{copy_repo, run_cupel};
use serde_json::{Value, json};

/// Runs each of `commands` as git in `dir`, as a fixed author.
fn git(dir: &Path, commands: &[&[&str]]) {
    for args in commands {
        let output = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(*args)
            .current_dir(dir)
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Whether the Rust suite joint's tools are installed; reported as skipped when not.
fn tools_installed() -> bool {
    let mut producers = ProducerRegistry::default();
    cupel_adapt::register(&mut producers);
    let lang = Lang {
        id: LangId::from("rust"),
        family: LangFamily::from("rust"),
    };
    let missing = producers
        .producers_for(FactKind::PerTestMap, &lang)
        .filter_map(
            |producer| match producer.availability(&Ctx::new(std::env::temp_dir())) {
                Availability::MissingTool { name, .. } => Some(name),
                _ => None,
            },
        )
        .next();
    match missing {
        Some(tool) => {
            eprintln!(
                "SKIPPED: {tool} not installed; the incremental-coverage fixture was not run"
            );
            false
        }
        None => true,
    }
}

/// A repository on `main` holding the fixture's base tree.
fn base() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &[&["init", "--quiet", "--initial-branch", "main"]]);
    copy_repo("incremental-coverage/rust/base", root);
    std::fs::write(root.join(".gitignore"), "/target\n").unwrap();
    git(
        root,
        &[&["add", "--all"], &["commit", "--quiet", "-m", "base"]],
    );
    dir
}

/// Branch `work` with one commit that adds a comment inside `scale` and nothing else.
fn advance(root: &Path) {
    git(root, &[&["checkout", "--quiet", "-b", "work"]]);
    copy_repo("incremental-coverage/rust/head", root);
    git(
        root,
        &[&["commit", "--quiet", "--all", "-m", "comment inside scale"]],
    );
}

/// `cupel <args>` in `root`, which must exit 0, as JSON, and its outcome.
fn check(root: &Path, args: &[&str]) -> (Value, cupel_fixtures::Outcome) {
    let outcome = run_cupel(args, root);
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    (serde_json::to_value(outcome.envelope()).unwrap(), outcome)
}

/// How the run's Rust suite was obtained.
fn suite(envelope: &Value) -> Value {
    envelope["run"]["suite"]["rust"].clone()
}

/// `tests.coverage`'s result in `envelope`: its metrics and score.
fn coverage(envelope: &Value) -> (Value, Value) {
    let check = &envelope["checks"][0];
    assert_eq!(check["id"], json!("tests.coverage"), "{envelope}");
    (check["metrics"].clone(), check["score"].clone())
}

#[test]
fn a_change_inside_one_function_reruns_its_test_alone_and_grades_as_the_whole_run_does() {
    if !tools_installed() {
        return;
    }
    let repo = base();
    let root = repo.path();
    let tree = ["check", "--only", "tests.coverage", "--format", "json"];
    let (whole, _) = check(root, &tree);
    assert_eq!(
        suite(&whole),
        json!({"mode": "full", "rerun": 4, "reused": 0})
    );
    advance(root);
    let (ranged, outcome) = check(
        root,
        &[
            "check",
            "--base",
            "main",
            "--only",
            "tests.coverage",
            "--format",
            "json",
        ],
    );
    assert_eq!(
        suite(&ranged),
        json!({"mode": "incremental", "rerun": 1, "reused": 3})
    );
    // The range grades `scale` alone, which `scales` covers in full.
    let (metrics, score) = coverage(&ranged);
    assert_eq!(
        (
            metrics["line"].clone(),
            metrics["uncovered"].clone(),
            score["value"].clone()
        ),
        (json!(1.0), json!(0.0), json!(1.0))
    );
    // The merged run, served from the cache for the whole tree, grades it as a fresh whole run does.
    let (merged, _) = check(root, &tree);
    std::fs::remove_dir_all(root.join(".cupel/cache")).unwrap();
    let (fresh, _) = check(root, &tree);
    assert_eq!(
        (suite(&merged), suite(&fresh), coverage(&merged)),
        (
            json!({"mode": "cached", "rerun": 0, "reused": 4}),
            json!({"mode": "full", "rerun": 4, "reused": 0}),
            coverage(&fresh)
        )
    );
    outcome.assert_golden("incremental-coverage/rust");
}
