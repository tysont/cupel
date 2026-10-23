// ABOUTME: Tests for scripts/heavy.sh, which `make dogfood` and every external suite run go through: one heavy
// ABOUTME: command at a time across the machine, under a lock file, and none from a working tree that no longer exists.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use tempfile::TempDir;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn script() -> PathBuf {
    workspace().join("scripts/heavy.sh")
}

/// `scripts/heavy.sh ARGS` in `dir`, locking `lock`.
fn heavy(dir: &Path, lock: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("sh");
    command
        .arg(script())
        .args(args)
        .env("CUPEL_HEAVY_LOCK", lock)
        .current_dir(dir);
    command
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// A fresh git repository, as `make dogfood` runs in one.
fn repository() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(dir.path())
            .status()
            .unwrap()
            .success()
    );
    dir
}

#[test]
fn the_script_passes_shellcheck() {
    if cupel_core::find_program("shellcheck").is_none() {
        eprintln!("SKIPPED: shellcheck not installed; this test did not run");
        return;
    }
    let output = Command::new("shellcheck")
        .args(["--shell=sh", "scripts/heavy.sh"])
        .current_dir(workspace())
        .output()
        .unwrap();
    assert_eq!(
        (output.status.code(), text(&output.stdout).as_str()),
        (Some(0), "")
    );
}

#[test]
fn make_dogfood_runs_through_the_script() {
    let makefile = fs::read_to_string(workspace().join("Makefile")).unwrap();
    // The recipe's grading line (its import of the baseline, ADR-140, writes only the store).
    let recipe = makefile
        .lines()
        .skip_while(|line| !line.starts_with("dogfood:"))
        .skip(1)
        .take_while(|line| line.starts_with('\t'))
        .find(|line| line.contains(" -- check "))
        .expect("a dogfood recipe line that runs check");
    assert_eq!(
        recipe.split_whitespace().take(2).collect::<Vec<_>>(),
        ["scripts/heavy.sh", "$(CARGO)"]
    );
}

#[test]
fn the_command_runs_with_the_lock_held_and_its_exit_code_is_returned() {
    let repo = repository();
    let lock = repo.path().join("lock");
    let output: Output = heavy(repo.path(), &lock, &["sh", "-c", "echo graded; exit 3"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(text(&output.stdout), "graded\n");
    assert!(lock.exists(), "the lock file is created");
}

#[test]
fn a_second_command_waits_until_the_first_releases_the_lock() {
    let repo = repository();
    let lock = repo.path().join("lock");
    let log = repo.path().join("log");
    // Each run appends its start and end; with the lock held, no run starts before the other ends.
    let body = format!(
        "echo start $1 >> {log}; sleep 1; echo end $1 >> {log}",
        log = log.display()
    );
    let mut first = heavy(repo.path(), &lock, &["sh", "-c", &body, "sh", "first"])
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let second = heavy(repo.path(), &lock, &["sh", "-c", &body, "sh", "second"])
        .output()
        .unwrap();
    assert!(first.wait().unwrap().success());
    assert!(second.status.success());
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        "start first\nend first\nstart second\nend second\n"
    );
    assert!(
        text(&second.stderr).contains("waiting for"),
        "the second run says it waits: {}",
        text(&second.stderr)
    );
}

#[test]
fn a_command_from_a_deleted_working_tree_refuses_to_start() {
    let repo = repository();
    let lock = tempfile::tempdir().unwrap();
    let gone = repo.path().join("worktree");
    fs::create_dir(&gone).unwrap();
    // The shell starts in the directory, which is removed before the script runs, as a checkout
    // deleted under a still-running agent's shell is.
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "rmdir '{}' && sh '{}' sh -c 'echo ran'",
            gone.display(),
            script().display()
        ))
        .env("CUPEL_HEAVY_LOCK", lock.path().join("lock"))
        .current_dir(&gone)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(text(&output.stdout), "");
    // The shell that lost its directory complains on its own line first.
    assert_eq!(
        text(&output.stderr).lines().last(),
        Some("heavy.sh: the working tree this command was started from no longer exists")
    );
}
