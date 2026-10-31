// ABOUTME: Tests for scripts/heavy.sh, which `make dogfood` and every external suite run go through: one heavy
// ABOUTME: command at a time across the machine, under a lock file, and none from a working tree that no longer exists.
mod support;

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;

use support::{HANG, eventually, recipe, workspace};
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
        .env_remove("CUPEL_HEAVY_HELD")
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

/// In an agent checkout `make check` compiles everything it runs in one step under the lock, before
/// its lint, tests, and docs run outside it against what was built (ADR-156 §3); elsewhere it takes
/// no lock.
#[test]
fn make_check_builds_under_the_lock_in_an_agent_checkout_before_it_tests() {
    let check = recipe("check");
    let locked = check
        .iter()
        .position(|line| {
            line == "$(if $(AGENT_CHECKOUT),scripts/heavy.sh $(MAKE) --no-print-directory check-build)"
        })
        .expect("the check's build step through the script in an agent checkout");
    let tested = check
        .iter()
        .position(|line| {
            line == "@scripts/tmpdir-guard.sh $(MAKE) --no-print-directory lint test doc"
        })
        .expect("lint, test, and doc");
    assert!(locked < tested, "{check:#?}");
    assert_eq!(
        recipe("check-build"),
        [
            "$(CARGO) clippy --workspace --all-targets -- -D warnings",
            "$(CARGO) test --workspace --no-run",
            "RUSTDOCFLAGS=\"-D warnings\" $(CARGO) doc --workspace --no-deps",
        ]
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

/// A repository whose lock a running holder holds: the lock, the log the runs append to, the file
/// whose creation releases every holder, and the holder, whose start is in the log.
struct Held {
    repo: TempDir,
    lock: PathBuf,
    log: PathBuf,
    release: PathBuf,
    holder: Child,
}

fn held_lock(name: &str) -> Held {
    let repo = repository();
    let lock = repo.path().join("lock");
    let log = repo.path().join("log");
    let release = repo.path().join("release");
    let holder = holder(repo.path(), &lock, &log, &release, name)
        .spawn()
        .unwrap();
    let started = format!("start {name}\n");
    assert!(
        eventually(|| fs::read_to_string(&log).is_ok_and(|log| log == started)),
        "the {name} run never started"
    );
    Held {
        repo,
        lock,
        log,
        release,
        holder,
    }
}

/// The first line a spawned run writes to stderr, within `HANG`.
fn first_stderr_line(child: &mut Child) -> String {
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let (said, heard) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = stderr.read_line(&mut line);
        let _ = said.send(line);
    });
    heard.recv_timeout(HANG).unwrap_or_default()
}

#[test]
fn a_second_command_waits_until_the_first_releases_the_lock() {
    let mut held = held_lock("first");
    let mut second = holder(
        held.repo.path(),
        &held.lock,
        &held.log,
        &held.release,
        "second",
    )
    .spawn()
    .unwrap();
    // The first run is released once the second has said it waits for it.
    let waiting = first_stderr_line(&mut second);
    fs::write(&held.release, "").unwrap();
    assert!(held.holder.wait().unwrap().success());
    assert!(second.wait().unwrap().success());
    assert_eq!(
        fs::read_to_string(&held.log).unwrap(),
        "start first\nend first\nstart second\nend second\n"
    );
    assert!(
        waiting.contains("waiting for"),
        "the second run says it waits: {waiting}"
    );
}

/// The integrator's merge chains set `CUPEL_HEAVY_PRIORITY=first`; a waiting run with it takes the
/// lock before any waiting run without it, however long the other has waited.
#[test]
fn a_prioritized_command_takes_the_lock_before_a_longer_waiting_default_one() {
    let Held {
        repo,
        lock,
        log,
        release,
        holder: mut holding,
    } = held_lock("holding");
    let (lock, log, release) = (&lock, &log, &release);
    let mut default = holder(repo.path(), lock, log, release, "default")
        .spawn()
        .unwrap();
    assert!(first_stderr_line(&mut default).contains("waiting for"));
    let mut first = holder(repo.path(), lock, log, release, "first")
        .env("CUPEL_HEAVY_PRIORITY", "first")
        .spawn()
        .unwrap();
    assert!(first_stderr_line(&mut first).contains("waiting for"));
    fs::write(release, "").unwrap();
    assert!(holding.wait().unwrap().success());
    assert!(first.wait().unwrap().success());
    assert!(default.wait().unwrap().success());
    assert_eq!(
        fs::read_to_string(log).unwrap(),
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

/// A command under the lock that runs another heavy command on the same lock (a check inside a
/// chain that holds it, a dogfood's check) runs it at once instead of waiting on itself (ADR-156 §4).
#[test]
fn a_command_on_the_lock_its_caller_holds_runs_at_once() {
    let repo = repository();
    let lock = repo.path().join("lock");
    let nested = format!("sh '{}' sh -c 'echo nested; exit 4'", script().display());
    let outer = heavy(repo.path(), &lock, &["sh", "-c", &nested])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (said, heard) = mpsc::channel();
    std::thread::spawn(move || said.send(outer.wait_with_output().unwrap()));
    let output = heard
        .recv_timeout(HANG)
        .expect("the nested command runs without waiting on its caller");
    assert_eq!(
        (
            output.status.code(),
            text(&output.stdout),
            text(&output.stderr)
        ),
        (Some(4), "nested\n".to_owned(), String::new())
    );
}

/// Holding one lock is no claim on another: a nested command naming a lock someone else holds waits
/// for it as any command does.
#[test]
fn a_command_on_another_lock_than_its_callers_still_waits() {
    let mut held = held_lock("holding");
    let other = held.repo.path().join("other-lock");
    let nested = format!(
        "CUPEL_HEAVY_LOCK='{}' sh '{}' sh -c 'echo end nested >> {}'",
        held.lock.display(),
        script().display(),
        held.log.display()
    );
    let mut outer = heavy(held.repo.path(), &other, &["sh", "-c", &nested])
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let waiting = first_stderr_line(&mut outer);
    fs::write(&held.release, "").unwrap();
    assert!(held.holder.wait().unwrap().success());
    assert!(outer.wait().unwrap().success());
    assert!(waiting.contains("waiting for"), "{waiting}");
    assert_eq!(
        fs::read_to_string(&held.log).unwrap(),
        "start holding\nend holding\nend nested\n"
    );
}
