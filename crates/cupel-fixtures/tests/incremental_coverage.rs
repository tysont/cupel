// ABOUTME: The suite joint's incremental run end to end (ADR-149, ADR-151): a commit changes one function of a Rust crate
// ABOUTME: whose suite ran whole once; the `--base` run reruns one test, keeps four, the binary's by its attribution.
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
        .find_map(
            |producer| match producer.availability(&Ctx::new(std::env::temp_dir())) {
                Availability::MissingTool { name, .. } => Some(name),
                _ => None,
            },
        );
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

/// Branch `work` with one commit that adds a comment inside `scale` and nothing else. The
/// changed file is written anew, as a checkout writes it: a copy keeps the fixture's old
/// modification time, which the suite cache's record would trust as unchanged (ADR-079).
fn advance(root: &Path) {
    git(root, &[&["checkout", "--quiet", "-b", "work"]]);
    copy_repo("incremental-coverage/rust/head", root);
    let changed = root.join("src/lib.rs");
    let text = std::fs::read(&changed).unwrap();
    std::fs::write(&changed, text).unwrap();
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

/// The result of check `id` in `envelope`.
fn result<'a>(envelope: &'a Value, id: &str) -> &'a Value {
    envelope["checks"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|check| check["id"] == json!(id))
        .unwrap_or_else(|| panic!("{id} ran: {envelope}"))
}

/// `tests.coverage`'s result in `envelope`: its metrics and score.
fn coverage(envelope: &Value) -> (Value, Value) {
    let check = result(envelope, "tests.coverage");
    (check["metrics"].clone(), check["score"].clone())
}

/// The checks a run selects: coverage, and mutation, whose engine needs the baseline and wants
/// the per-test map, so the suite facts are reached and the joint runs the suite once (ADR-084
/// §2) and narrows each mutant to its covering tests (ADR-083 §6).
const ONLY: [&str; 4] = ["--only", "tests.coverage", "--only", "tests.mutation"];

#[test]
fn a_change_inside_one_function_reruns_its_test_alone_and_grades_as_the_whole_run_does() {
    if !tools_installed() {
        return;
    }
    let repo = base();
    let root = repo.path();
    let tree: Vec<&str> = ["check"]
        .into_iter()
        .chain(ONLY)
        .chain(["--format", "json"])
        .collect();
    let (whole, _) = check(root, &tree);
    assert_eq!(
        suite(&whole),
        json!({"mode": "full", "rerun": 5, "reused": 0, "tagged": 0})
    );
    advance(root);
    let ranged_args: Vec<&str> = ["check", "--base", "main"]
        .into_iter()
        .chain(ONLY)
        .chain(["--format", "json"])
        .collect();
    let (ranged, outcome) = check(root, &ranged_args);
    // `the_binary_scales_each_pair_of_its_input` ran `scale` through the binary; its attribution
    // keeps it, since none of its files changed.
    assert_eq!(
        suite(&ranged),
        json!({"mode": "incremental", "rerun": 1, "reused": 4, "tagged": 1})
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
    // The merged run, served from the cache for the whole tree, grades it as a fresh whole run
    // does: no cache and no instrumented build, as a new checkout has.
    let (merged, _) = check(root, &tree);
    std::fs::remove_dir_all(root.join(".cupel/cache")).unwrap();
    std::fs::remove_dir_all(root.join("target/cupel-llvm-cov")).unwrap();
    let (fresh, _) = check(root, &tree);
    assert_eq!(
        (suite(&merged), suite(&fresh), coverage(&merged)),
        (
            json!({"mode": "cached", "rerun": 0, "reused": 5, "tagged": 0}),
            json!({"mode": "full", "rerun": 5, "reused": 0, "tagged": 0}),
            coverage(&fresh)
        )
    );
    outcome.assert_golden("incremental-coverage/rust");
}

/// `tests.mutation`'s metric `name` in `envelope`.
fn mutation(envelope: &Value, name: &str) -> Value {
    result(envelope, "tests.mutation")["metrics"][name].clone()
}

#[test]
fn a_second_change_reuses_the_mutants_of_the_function_the_first_change_touched() {
    if !tools_installed() {
        return;
    }
    let repo = base();
    let root = repo.path();
    let range: Vec<&str> = ["check", "--base", "main"]
        .into_iter()
        .chain(ONLY)
        .chain(["--format", "json"])
        .collect();
    // As the inner loop's cold run: the first range run has no cache, so it runs whole.
    advance(root);
    let (first, _) = check(root, &range);
    assert_eq!(suite(&first)["mode"], json!("full"));
    // A second commit inside `negate`, as the inner loop's warm change follows its cold one.
    let lib = root.join("src/lib.rs");
    let text = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(
        &lib,
        text.replace("    -value\n", "    // The negation.\n    -value\n"),
    )
    .unwrap();
    git(
        root,
        &[&["commit", "--quiet", "--all", "-m", "comment inside negate"]],
    );
    let (second, _) = check(root, &range);
    assert_eq!(
        (
            suite(&second)["mode"].clone(),
            mutation(&second, "reused"),
            mutation(&first, "executed")
        ),
        (
            json!("incremental"),
            mutation(&first, "executed"),
            mutation(&first, "executed")
        ),
        "`scale`'s mutants, unchanged since the first range run, are reused: {}",
        second["checks"]
    );
}

#[test]
fn a_change_to_the_data_an_attribution_names_reruns_the_binarys_test_alone() {
    if !tools_installed() {
        return;
    }
    let repo = base();
    let root = repo.path();
    let range: Vec<&str> = ["check", "--base", "main"]
        .into_iter()
        .chain(ONLY)
        .chain(["--format", "json"])
        .collect();
    advance(root);
    let (first, _) = check(root, &range);
    assert_eq!(suite(&first)["mode"], json!("full"));
    // The data the binary's test hands it, each pair's factors swapped: no test's coverage
    // names the file, its attribution does, and without it nothing would rerun.
    std::fs::write(root.join("tests/data/pairs.txt"), "3 2\n2 -4\n").unwrap();
    git(
        root,
        &[&["commit", "--quiet", "--all", "-m", "factors swapped"]],
    );
    let (second, outcome) = check(root, &range);
    assert_eq!(
        suite(&second),
        json!({"mode": "incremental", "rerun": 1, "reused": 4, "tagged": 0})
    );
    outcome.assert_golden("incremental-coverage/rust-attributed");
}
