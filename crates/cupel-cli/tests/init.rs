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
fn init_write_creates_exactly_the_previewed_files_and_its_run_record() {
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
            ".cupel/runs/".to_owned(),
            format!(".cupel/runs/{id}.json"),
            "Cargo.toml".to_owned(),
            "cupel.toml".to_owned(),
        ]
    );
    for (path, diff) in preview {
        assert_eq!(
            std::fs::read_to_string(repo.path(&path)).unwrap(),
            added(&diff),
            "{path}"
        );
    }
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
    let config = added(&writes[0].1);
    assert!(config.contains("default_branch = \"trunk\"\n"), "{config}");
}

#[test]
fn init_outside_git_writes_no_default_branch() {
    let writes = would_write(&cargo_repo().run(&["init"]));
    assert!(!added(&writes[0].1).contains("default_branch"));
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

/// The `init.unsupported-root` findings of an envelope, as `(subject, severity, title)`.
fn unsupported_roots(outcome: &Outcome) -> Vec<(String, String, String)> {
    outcome.envelope()["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["kind"] == json!("init.unsupported-root"))
        .map(|finding| {
            (
                finding["subject"].as_str().unwrap_or_default().to_owned(),
                finding["severity"].as_str().unwrap().to_owned(),
                finding["title"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn init_leaves_java_and_clojure_roots_out_of_roots_and_reports_them() {
    let repo = cargo_repo();
    repo.write("svc/pom.xml", "<project/>\n");
    repo.write("tools/deps.edn", "{}\n");

    let preview = repo.run(&["init"]);
    assert_eq!(preview.code, 0, "{}", preview.stdout);
    let text = added(&would_write(&preview)[0].1);
    assert!(
        !text.contains("java") && !text.contains("clojure"),
        "{text}"
    );
    let expected = [
        (
            "info".to_owned(),
            "unsupported language root: java at ./svc — supported in a later release".to_owned(),
        ),
        (
            "info".to_owned(),
            "unsupported language root: clojure at ./tools — supported in a later release"
                .to_owned(),
        ),
    ];
    let shape = |outcome: &Outcome| -> Vec<(String, String)> {
        unsupported_roots(outcome)
            .into_iter()
            .map(|(_, severity, title)| (severity, title))
            .collect()
    };
    assert_eq!(shape(&preview), expected);
    let finding = &preview.envelope()["findings"][0];
    assert_eq!(finding["check"], json!("init"));
    assert_eq!(finding["action"]["kind"], json!("review"));
    assert_eq!(
        finding["evidence"],
        json!({"lang": "java", "path": "svc", "build_files": ["pom.xml"]})
    );

    let written = repo.run(&["init", "--write"]);
    assert_eq!(written.code, 0, "{}", written.stdout);
    assert_eq!(shape(&written), expected);
    let doctor = repo.run(&["doctor"]);
    assert!(
        !doctor.stdout.contains("doctor.config-invalid"),
        "the generated cupel.toml validates: {}",
        doctor.stdout
    );
}
