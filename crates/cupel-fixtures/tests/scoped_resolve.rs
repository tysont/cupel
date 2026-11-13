// ABOUTME: Resolved findings under a scoped run end to end (ADR-206): a baseline over the whole tree, then a change
// ABOUTME: to one of two files resolves only the findings on the symbols it touched, in `check` and `baseline diff`.
use std::path::Path;
use std::process::Command;

use cupel_core::Envelope;
use cupel_fixtures::{copy_repo, run_cupel};

/// Runs git in `dir` as a fixed author.
fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Runs `cupel <args> --format json` in `dir` and returns its envelope, which must exit `code`.
fn cupel(dir: &Path, args: &[&str], code: i32) -> Envelope {
    let args: Vec<&str> = args.iter().copied().chain(["--format", "json"]).collect();
    let outcome = run_cupel(&args, dir);
    assert_eq!(outcome.code, Some(code), "{}", outcome.stdout);
    outcome.envelope().clone()
}

/// Each finding's file and whether it is new.
fn files(envelope: &Envelope) -> Vec<(String, bool)> {
    envelope
        .findings
        .iter()
        .map(|finding| {
            let file = finding.location.as_ref().map(|at| at.file.clone());
            (file.unwrap_or_default(), finding.new_since_baseline)
        })
        .collect()
}

/// The fixture's base commit on `main`, its whole tree checked and set as the baseline, then
/// branch `work` committing the head `head`'s files. Returns the changed-scope check and the
/// `baseline diff` after it.
fn changed_after_baseline(dir: &Path, head: &str) -> (Envelope, Envelope) {
    git(dir, &["init", "--quiet", "--initial-branch", "main"]);
    copy_repo("scoped-resolve/base", dir);
    git(dir, &["add", "--all"]);
    git(dir, &["commit", "--quiet", "-m", "names"]);
    let tree = cupel(dir, &["check", "--only", "static.complexity"], 0);
    assert_eq!(files(&tree).len(), 2, "{:?}", files(&tree));
    cupel(dir, &["baseline", "set", "--write"], 0);
    git(dir, &["checkout", "--quiet", "-b", "work"]);
    copy_repo(&format!("scoped-resolve/{head}"), dir);
    git(dir, &["commit", "--quiet", "--all", "-m", head]);
    let changed = ["check", "--changed", "--only", "static.complexity"];
    let check = cupel(dir, &changed, 0);
    (check, cupel(dir, &["baseline", "diff"], 0))
}

#[test]
fn a_change_to_one_file_resolves_nothing_in_the_file_it_did_not_touch() {
    let dir = tempfile::tempdir().unwrap();
    let (check, diff) = changed_after_baseline(dir.path(), "touched");

    // The change touched `shout` only, so the run left `name_a` out and reported nothing.
    assert_eq!((check.summary.new, check.summary.resolved), (0, 0));
    assert_eq!(files(&check), []);
    assert_eq!(
        check
            .run
            .scope
            .as_ref()
            .map(|scope| scope.symbols_left_out.clone()),
        Some(vec!["a.name_a".to_owned()])
    );
    assert_eq!((diff.summary.new, diff.summary.resolved), (0, 0));
    assert_eq!(files(&diff), []);
}

#[test]
fn a_change_that_fixes_a_finding_resolves_exactly_that_finding() {
    let dir = tempfile::tempdir().unwrap();
    let (check, diff) = changed_after_baseline(dir.path(), "fixed");

    assert_eq!((check.summary.new, check.summary.resolved), (0, 1));
    assert_eq!(files(&check), []);
    assert_eq!((diff.summary.new, diff.summary.resolved), (0, 1));
    assert_eq!(files(&diff), [("src/a.py".to_owned(), false)]);
}
