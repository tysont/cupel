// ABOUTME: `perf.footprint` end to end (ADR-103): on a git history whose branch adds a vulnerable dependency, --changed
// ABOUTME: reports the vulnerability and the growth against the merge base, and a tree run reports every dependency.
use std::fs;
use std::path::Path;
use std::process::Command;

use cupel_core::{CheckStatus, Envelope};
use cupel_fixtures::{copy_repo, run_cupel};

const CHECK: &str = "perf.footprint";

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn check(dir: &Path, scope: &[&str]) -> Envelope {
    let mut args = vec!["check", "--only", CHECK, "--format", "json"];
    args.extend(scope);
    let outcome = run_cupel(&args, dir);
    assert_eq!(outcome.code, Some(0), "{}", outcome.stderr);
    outcome.envelope().clone()
}

/// (kind, package or `-`) of every finding, sorted.
fn kinds(envelope: &Envelope) -> Vec<(String, String)> {
    let mut kinds: Vec<(String, String)> = envelope
        .findings
        .iter()
        .map(|finding| {
            let package = finding.evidence.0.get("package").and_then(|p| p.as_str());
            (finding.kind.clone(), package.unwrap_or("-").to_owned())
        })
        .collect();
    kinds.sort();
    kinds
}

/// The Go fixture with `main` lacking `example.com/oldlib`, and branch `work` adding it with a
/// change to the code, since a range runs a language's checks only when its source changed.
fn history() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("perf.footprint/go", dir.path());
    let gomod = dir.path().join("go.mod");
    let head = fs::read_to_string(&gomod).unwrap();
    fs::write(
        &gomod,
        head.replace("\texample.com/oldlib v2.0.0 // indirect\n", ""),
    )
    .unwrap();
    git(dir.path(), &["init", "--quiet", "--initial-branch", "main"]);
    git(dir.path(), &["add", "--all"]);
    git(dir.path(), &["commit", "--quiet", "-m", "base"]);
    let base = git(dir.path(), &["rev-parse", "HEAD"]);
    git(dir.path(), &["checkout", "--quiet", "-b", "work"]);
    fs::write(&gomod, head).unwrap();
    let code = dir.path().join("ledger.go");
    let text = fs::read_to_string(&code).unwrap();
    fs::write(&code, text.replace("amount + 1", "amount + 2")).unwrap();
    git(
        dir.path(),
        &["commit", "--quiet", "--all", "-m", "add oldlib"],
    );
    (dir, base)
}

#[test]
fn changed_reports_the_added_vulnerable_dependency_and_the_growth() {
    let (dir, base) = history();
    let envelope = check(dir.path(), &["--changed"]);
    assert_eq!(
        kinds(&envelope),
        [
            ("dep.vuln".to_owned(), "example.com/oldlib".to_owned()),
            ("footprint.grew".to_owned(), "-".to_owned()),
        ]
    );
    let grew = envelope
        .findings
        .iter()
        .find(|finding| finding.kind == "footprint.grew")
        .unwrap();
    assert_eq!(
        grew.evidence.0["added"],
        serde_json::json!(["example.com/oldlib@v2.0.0"])
    );
    assert_eq!(
        grew.repro,
        format!("cupel check --only {CHECK} --base {base}")
    );
    assert_eq!(envelope.checks[0].status, CheckStatus::Fail);
}

#[test]
fn a_tree_run_reports_every_dependency_and_no_growth() {
    let (dir, _) = history();
    let envelope = check(dir.path(), &[]);
    assert_eq!(
        kinds(&envelope),
        [("dep.vuln".to_owned(), "example.com/oldlib".to_owned())]
    );
}
