// ABOUTME: Tests for scripts/worktree.sh, which creates agent worktrees under the main checkout's .claude/worktrees
// ABOUTME: and removes them with their build and their mutation slots (ADR-156 §5, ADR-159).
mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use support::{git, main_checkout, workspace};
use tempfile::TempDir;

/// `scripts/worktree.sh ARGS` in `dir`, with `tmp` as the temporary directory the slots live under.
fn worktree(dir: &Path, tmp: &Path, args: &[&str]) -> Output {
    Command::new("sh")
        .arg(workspace().join("scripts/worktree.sh"))
        .args(args)
        .env("TMPDIR", tmp)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn succeeded(output: &Output) -> bool {
    output.status.success()
}

/// A main checkout and the agent worktree `name` the script added to it.
fn added(name: &str, tmp: &Path) -> (TempDir, PathBuf) {
    let main = main_checkout();
    let output = worktree(main.path(), tmp, &["add", name]);
    assert!(succeeded(&output), "{}", text(&output.stderr));
    let path = main
        .path()
        .canonicalize()
        .unwrap()
        .join(".claude/worktrees")
        .join(name);
    (main, path)
}

#[test]
fn the_script_passes_shellcheck() {
    if !support::installed("shellcheck") {
        return;
    }
    let output = Command::new("shellcheck")
        .args(["--shell=sh", "scripts/worktree.sh"])
        .current_dir(workspace())
        .output()
        .unwrap();
    assert_eq!(
        (output.status.code(), text(&output.stdout)),
        (Some(0), String::new())
    );
}

/// A branch named like a release tag makes pushing the tag ambiguous, so `add` makes none.
#[test]
fn add_refuses_a_name_like_a_release_tag() {
    let tmp = TempDir::new().unwrap();
    let main = main_checkout();
    let output = worktree(main.path(), tmp.path(), &["add", "v1.2.3"]);
    assert_eq!(
        (output.status.code(), text(&output.stderr)),
        (
            Some(1),
            "release.sh: \"v1.2.3\" is named like a release tag (v*): a branch of that name makes pushing the tag ambiguous\n".to_owned()
        )
    );
    assert_eq!(git(main.path(), &["branch", "--list", "v*"]), "");
}

/// `add NAME` makes `.claude/worktrees/NAME` in the main checkout on a new branch `NAME` from `main`.
#[test]
fn add_makes_a_worktree_on_a_new_branch_from_main() {
    let tmp = TempDir::new().unwrap();
    let (main, path) = added("b9-thing", tmp.path());
    assert_eq!(git(&path, &["branch", "--show-current"]), "b9-thing");
    assert_eq!(
        git(&path, &["rev-parse", "HEAD"]),
        git(main.path(), &["rev-parse", "main"])
    );
}

/// From inside another worktree, `add` still puts the new one under the main checkout, and an
/// existing branch is checked out rather than made again.
#[test]
fn add_from_a_worktree_checks_out_an_existing_branch_under_the_main_checkout() {
    let tmp = TempDir::new().unwrap();
    let (main, first) = added("first", tmp.path());
    git(main.path(), &["branch", "existing"]);
    let output = worktree(&first, tmp.path(), &["add", "existing"]);
    assert!(succeeded(&output), "{}", text(&output.stderr));
    let second = main
        .path()
        .canonicalize()
        .unwrap()
        .join(".claude/worktrees/existing");
    assert_eq!(git(&second, &["branch", "--show-current"]), "existing");
}

/// `remove NAME` deletes the worktree with its ignored build and its mutation slots, which live in its own
/// `.cupel/work/slots/` (ADR-159), and leaves the branch.
#[test]
fn remove_deletes_the_worktree_its_build_and_its_slots_and_keeps_the_branch() {
    let tmp = TempDir::new().unwrap();
    let (main, path) = added("done", tmp.path());
    for (dir, file) in [
        ("target/cupel-llvm-cov", "build"),
        (".cupel/work/slots/rust/0/target/debug", "cupel"),
    ] {
        fs::create_dir_all(path.join(dir)).unwrap();
        fs::write(path.join(dir).join(file), "built\n").unwrap();
    }
    let output = worktree(main.path(), tmp.path(), &["remove", "done"]);
    assert!(succeeded(&output), "{}", text(&output.stderr));
    assert!(!path.exists());
    assert_eq!(git(main.path(), &["branch", "--list", "done"]), "done");
    assert!(!git(main.path(), &["worktree", "list"]).contains("done"));
}

/// Removing the worktree a shell stands in would leave that shell nowhere; the script refuses.
#[test]
fn remove_refuses_to_remove_the_worktree_it_runs_in() {
    let tmp = TempDir::new().unwrap();
    let (_main, path) = added("here", tmp.path());
    let output = worktree(&path, tmp.path(), &["remove", "here"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("from inside"),
        "{}",
        text(&output.stderr)
    );
    assert!(path.exists());
}
