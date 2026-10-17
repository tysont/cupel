// ABOUTME: Integration tests for `cupel init` (ADR-021): previews and dry runs that write nothing,
// ABOUTME: `--write` creating exactly its files, `--force`, and `default_branch` from git.
mod support;

use std::process::Command as Process;

use serde_json::{Value, json};
use support::{Outcome, Repo};

const MANIFEST: &str = "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n";

/// A repository holding one Cargo crate.
fn cargo_repo() -> Repo {
    let repo = Repo::new();
    repo.write("Cargo.toml", MANIFEST);
    repo
}

/// The planned writes of a plan envelope, as `(path, diff)` pairs.
fn would_write(outcome: &Outcome) -> Vec<(String, String)> {
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("plan"), "{}", outcome.stdout);
    envelope["plan"]["would_write"]
        .as_array()
        .unwrap()
        .iter()
        .map(|write| {
            (
                write["path"].as_str().unwrap().to_owned(),
                write["diff"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// The content a diff from `/dev/null` adds.
fn added(diff: &str) -> String {
    let mut text = String::new();
    for line in diff.lines() {
        if let Some(content) = line.strip_prefix('+').filter(|_| !line.starts_with("+++")) {
            text.push_str(content);
            text.push('\n');
        }
    }
    text
}

#[test]
fn init_without_write_previews_both_files_and_leaves_the_tree_byte_identical() {
    let repo = cargo_repo();
    let before = repo.tree();
    let outcome = repo.run(&["init"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let writes = would_write(&outcome);
    let paths: Vec<&str> = writes.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(paths, ["cupel.toml", ".cupel/.gitignore"]);
    assert!(writes[0].1.starts_with("--- /dev/null\n+++ b/cupel.toml\n"));
    assert!(added(&writes[0].1).contains("test = \"cargo test --workspace\"\n"));
    assert_eq!(added(&writes[1].1), "*\n!suppressions.toml\n");
    assert_eq!(
        outcome.envelope()["next_actions"],
        json!([{"priority": 1, "finding": null, "kind": "configure",
                "summary": "Nothing was written; run `cupel init --write` to write these files.",
                "command": "cupel init --write"}])
    );
    assert_eq!(repo.tree(), before, "a preview changes nothing");
}

#[test]
fn init_dry_run_plans_exactly_what_the_preview_shows() {
    let repo = cargo_repo();
    let before = repo.tree();
    let preview = would_write(&repo.run(&["init"]));
    let dry_run = would_write(&repo.run(&["init", "--dry-run"]));
    assert_eq!(dry_run, preview);
    let with_write = repo.run(&["init", "--dry-run", "--write"]);
    assert_eq!(would_write(&with_write), preview);
    assert_eq!(repo.tree(), before);
}

#[test]
fn init_write_creates_exactly_the_previewed_files_its_run_record_and_the_store() {
    let repo = cargo_repo();
    let preview = would_write(&repo.run(&["init"]));
    let outcome = repo.run(&["init", "--write"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("pass"));
    let id = envelope["run"]["id"].as_str().unwrap();
    let names: Vec<String> = repo.tree().into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        names,
        [
            ".cupel/".to_owned(),
            ".cupel/.gitignore".to_owned(),
            ".cupel/cupel.db".to_owned(),
            ".cupel/cupel.lock".to_owned(),
            ".cupel/runs/".to_owned(),
            format!(".cupel/runs/{id}.json"),
            "Cargo.toml".to_owned(),
            "cupel.toml".to_owned(),
        ]
    );
    let written: Vec<(&str, String)> = preview
        .iter()
        .map(|(path, _)| {
            (
                path.as_str(),
                std::fs::read_to_string(repo.path(path)).unwrap(),
            )
        })
        .collect();
    let previewed: Vec<(&str, String)> = preview
        .iter()
        .map(|(path, diff)| (path.as_str(), added(diff)))
        .collect();
    assert_eq!(written, previewed);
}

/// Asserts `outcome` refused to overwrite `cupel.toml` and named `--force`.
fn assert_refused(outcome: &Outcome) {
    assert_eq!(outcome.code, 2, "{}", outcome.stdout);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    assert_eq!(finding["evidence"]["argument"], json!("--force"));
    assert_eq!(
        finding["id"],
        json!(support::cli_finding_id("cli.usage", "--force"))
    );
    let fix = finding["action"]["summary"].as_str().unwrap();
    assert!(fix.contains("--force"), "{fix}");
}

#[test]
fn init_refuses_to_overwrite_cupel_toml_without_force() {
    let repo = cargo_repo();
    repo.write("cupel.toml", "depth = \"quick\"\n");
    for args in [&["init"][..], &["init", "--dry-run"], &["init", "--write"]] {
        assert_refused(&repo.run(args));
    }
    assert_eq!(
        std::fs::read_to_string(repo.path("cupel.toml")).unwrap(),
        "depth = \"quick\"\n"
    );
}

#[test]
fn init_force_diffs_against_and_then_replaces_the_existing_file() {
    let repo = cargo_repo();
    repo.write("cupel.toml", "depth = \"quick\"\n");
    let writes = would_write(&repo.run(&["init", "--force"]));
    assert!(
        writes[0]
            .1
            .starts_with("--- a/cupel.toml\n+++ b/cupel.toml\n"),
        "{}",
        writes[0].1
    );
    assert!(
        writes[0].1.contains("\n-depth = \"quick\"\n"),
        "{}",
        writes[0].1
    );
    let outcome = repo.run(&["init", "--force", "--write"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let written = std::fs::read_to_string(repo.path("cupel.toml")).unwrap();
    assert!(written.contains("depth = \"standard\"\n"), "{written}");
}

#[test]
fn init_force_replaces_a_cupel_toml_that_does_not_load() {
    let repo = cargo_repo();
    repo.write("cupel.toml", "colour = \"blue\"\n");
    assert_refused(&repo.run(&["init"]));
    let outcome = repo.run(&["init", "--force", "--write"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let written = std::fs::read_to_string(repo.path("cupel.toml")).unwrap();
    assert!(!written.contains("colour"), "{written}");
}

/// The `default_branch` line of a generated `cupel.toml`, if it has one.
fn default_branch_line(config: &str) -> Option<String> {
    config
        .lines()
        .find(|line| line.starts_with("default_branch"))
        .map(str::to_owned)
}

#[test]
fn init_takes_default_branch_from_git() {
    let repo = cargo_repo();
    let git = Process::new("git")
        .args(["init", "--quiet", "--initial-branch", "trunk"])
        .current_dir(repo.root())
        .output();
    if !git.is_ok_and(|output| output.status.success()) {
        eprintln!("SKIPPED: git not installed");
        return;
    }
    let writes = would_write(&repo.run(&["init"]));
    assert_eq!(
        default_branch_line(&added(&writes[0].1)),
        Some("default_branch = \"trunk\"".to_owned())
    );
}

#[test]
fn init_outside_git_writes_no_default_branch() {
    let writes = would_write(&cargo_repo().run(&["init"]));
    assert_eq!(default_branch_line(&added(&writes[0].1)), None);
}

#[test]
fn init_describes_itself_without_a_pending_step() {
    let envelope = cargo_repo().run(&["init", "--describe"]).envelope();
    let description = &envelope["description"];
    assert_eq!(description["pending_step"], Value::Null);
    let names: Vec<&str> = description["arguments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|argument| argument["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["--force"]);
}

#[test]
fn init_writes_java_and_clojure_roots_with_their_runners_and_reports_no_unsupported_root() {
    let repo = cargo_repo();
    repo.write("svc/pom.xml", "<project/>\n");
    repo.write("tools/deps.edn", "{}\n");

    let preview = repo.run(&["init"]);
    assert_eq!(preview.code, 0, "{}", preview.stdout);
    let text = added(&would_write(&preview)[0].1);
    for expected in [
        "lang = \"java\"",
        "lang = \"clojure\"",
        "mvn -B --no-transfer-progress test",
        "clojure -M:test",
    ] {
        assert!(text.contains(expected), "{expected} in {text}");
    }
    let unsupported = preview.envelope()["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["kind"] == json!("init.unsupported-root"))
        .count();
    assert_eq!(unsupported, 0);

    let written = repo.run(&["init", "--write"]);
    assert_eq!(written.code, 0, "{}", written.stdout);
    let doctor = repo.run(&["doctor"]);
    assert!(
        !doctor.stdout.contains("doctor.config-invalid"),
        "the generated cupel.toml validates: {}",
        doctor.stdout
    );
}
