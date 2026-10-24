// ABOUTME: Tests for scripts/heavy.sh, which `make dogfood` and every external suite run go through: one heavy
// ABOUTME: command at a time across the machine, under a lock file, and none from a working tree that no longer exists.
mod support;

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;

use support::{HANG, eventually, workspace};
use tempfile::TempDir;

fn script() -> PathBuf {
    workspace().join("scripts/heavy.sh")
}

/// `scripts/heavy.sh ARGS` in `dir`, locking `lock`, at the default priority whatever the test's own
/// environment says (a merge chain runs the suite with `CUPEL_HEAVY_PRIORITY=first` set).
fn heavy(dir: &Path, lock: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("sh");
    command
        .arg(script())
        .args(args)
        .env("CUPEL_HEAVY_LOCK", lock)
        .env("CUPEL_HEAVY_PRIORITY", "default")
        .env("CUPEL_HEAVY_MIN_FREE_GB", "0")
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
    let release = repo.path().join("release");
    // Each run appends its start and end, and ends only once the test has made `release`; with
    // the lock held, no run starts before the other ends.
    let body = format!(
        "echo start $1 >> {log}; while [ ! -f {release} ]; do sleep 0.05; done; echo end $1 >> {log}",
        log = log.display(),
        release = release.display()
    );
    let mut first = heavy(repo.path(), &lock, &["sh", "-c", &body, "sh", "first"])
        .spawn()
        .unwrap();
    assert!(
        eventually(|| fs::read_to_string(&log).is_ok_and(|log| log == "start first\n")),
        "the first run never started"
    );
    let mut second = heavy(repo.path(), &lock, &["sh", "-c", &body, "sh", "second"])
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The first run is released once the second has said it waits for it.
    let mut stderr = BufReader::new(second.stderr.take().unwrap());
    let (said, heard) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = stderr.read_line(&mut line);
        let _ = said.send(line);
    });
    let waiting = heard.recv_timeout(HANG).unwrap_or_default();
    fs::write(&release, "").unwrap();
    assert!(first.wait().unwrap().success());
    assert!(second.wait().unwrap().success());
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        "start first\nend first\nstart second\nend second\n"
    );
    assert!(
        waiting.contains("waiting for"),
        "the second run says it waits: {waiting}"
    );
}

/// A run that holds the lock until `release` exists, logging its start and end as `name`.
fn holder(dir: &Path, lock: &Path, log: &Path, release: &Path, name: &str) -> Command {
    let body = format!(
        "echo start $1 >> {log}; while [ ! -f {release} ]; do sleep 0.05; done; echo end $1 >> {log}",
        log = log.display(),
        release = release.display()
    );
    let mut command = heavy(dir, lock, &["sh", "-c", &body, "sh", name]);
    command.stderr(Stdio::piped());
    command
}

/// The first line a spawned run writes to stderr, within `HANG`.
fn first_stderr_line(child: &mut std::process::Child) -> String {
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let (said, heard) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = stderr.read_line(&mut line);
        let _ = said.send(line);
    });
    heard.recv_timeout(HANG).unwrap_or_default()
}

/// The integrator's merge chains set `CUPEL_HEAVY_PRIORITY=first`; a waiting run with it takes the
/// lock before any waiting run without it, however long the other has waited.
#[test]
fn a_prioritized_command_takes_the_lock_before_a_longer_waiting_default_one() {
    let repo = repository();
    let lock = repo.path().join("lock");
    let log = repo.path().join("log");
    let release = repo.path().join("release");
    let mut holding = holder(repo.path(), &lock, &log, &release, "holding")
        .spawn()
        .unwrap();
    assert!(
        eventually(|| fs::read_to_string(&log).is_ok_and(|log| log == "start holding\n")),
        "the holding run never started"
    );
    let mut default = holder(repo.path(), &lock, &log, &release, "default")
        .spawn()
        .unwrap();
    assert!(first_stderr_line(&mut default).contains("waiting for"));
    let mut first = holder(repo.path(), &lock, &log, &release, "first")
        .env("CUPEL_HEAVY_PRIORITY", "first")
        .spawn()
        .unwrap();
    assert!(first_stderr_line(&mut first).contains("waiting for"));
    fs::write(&release, "").unwrap();
    assert!(holding.wait().unwrap().success());
    assert!(first.wait().unwrap().success());
    assert!(default.wait().unwrap().success());
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        "start holding\nend holding\nstart first\nend first\nstart default\nend default\n"
    );
}

/// A heavy command that runs out of disk corrupts what it was building and takes the machine's other
/// work with it, so the script refuses to start one below a floor of free space (40 GB by default;
/// `CUPEL_HEAVY_MIN_FREE_GB` moves it) and says what to run.
#[test]
fn a_command_refuses_to_start_below_the_free_space_floor() {
    let repo = repository();
    let lock = repo.path().join("lock");
    let output = heavy(repo.path(), &lock, &["sh", "-c", "echo ran"])
        .env("CUPEL_HEAVY_MIN_FREE_GB", "999999")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(text(&output.stdout), "");
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("GB free") && stderr.contains("make reclaim"),
        "the refusal names the floor and the remedy: {stderr}"
    );
    assert!(!lock.exists(), "no lock is taken");
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
