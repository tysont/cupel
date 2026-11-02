// ABOUTME: Tests for scripts/tmpdir-guard.sh, which `make check`'s suites and `make dogfood`'s gate run under: a temporary
// ABOUTME: directory of their own as TMPDIR, a failure naming whatever they left in it, and the dogfood's slots kept.
#![cfg(unix)]
mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use support::{assert_executable, assert_shellcheck_clean, recipe, workspace};
use tempfile::TempDir;

fn script() -> PathBuf {
    workspace().join("scripts/tmpdir-guard.sh")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `scripts/tmpdir-guard.sh sh -c SCRIPT` with `tmp` as the temporary directory it is called with.
fn guarded(tmp: &Path, script: &str) -> Output {
    Command::new("sh")
        .arg(self::script())
        .args(["sh", "-c", script])
        .env("TMPDIR", tmp)
        .output()
        .unwrap()
}

/// `scripts/tmpdir-guard.sh --slots-of REPO sh -c SCRIPT` with `tmp` as the caller's temporary
/// directory and `REPO` as the script's first argument.
fn guarded_keeping(tmp: &Path, repo: &Path, script: &str) -> Output {
    Command::new("sh")
        .arg(self::script())
        .arg("--slots-of")
        .arg(repo)
        .args(["sh", "-c", script, "sh"])
        .arg(repo.canonicalize().unwrap())
        .env("TMPDIR", tmp)
        .output()
        .unwrap()
}

/// Makes the slot directory `key` of the repository at `repo` under `tmp`, as the mutation engine
/// does (ADR-141 §3), with one file in its first slot.
fn slots(tmp: &Path, key: &str, repo: &Path) -> PathBuf {
    let dir = tmp.join("cupel-mutate").join(key);
    std::fs::create_dir_all(dir.join("rust/0")).unwrap();
    std::fs::write(dir.join("rust/0/built"), "built\n").unwrap();
    std::fs::write(
        dir.join("repository"),
        repo.canonicalize().unwrap().display().to_string(),
    )
    .unwrap();
    dir
}

/// The names of what `dir` holds.
fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// `make check` runs the script directly, so it is executable; and it passes `shellcheck`.
#[test]
fn the_script_is_executable_and_passes_shellcheck() {
    assert_executable("scripts/tmpdir-guard.sh");
    assert_shellcheck_clean("scripts/tmpdir-guard.sh");
}

/// The command runs with a fresh directory under the caller's as its TMPDIR, which is gone after it. Its name is
/// short, `c.` and six characters: a guarded mutation run nests its own TMPDIR and a test's repository below it, and
/// the socket a daemon test makes in that repository must still fit macOS's 103-character socket path (canonical
/// `$TMPDIR` 57, this 9, the run's 7, the repository's 11, `.cupel/serve.sock` 17: 101).
#[test]
fn a_command_that_leaves_nothing_passes_and_its_temporary_directory_goes() {
    let tmp = TempDir::new().unwrap();
    let output = guarded(
        tmp.path(),
        r#"made=$(mktemp -d) && rmdir "$made" && printf '%s' "$TMPDIR""#,
    );
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let inner = PathBuf::from(text(&output.stdout));
    assert_eq!(inner.parent(), Some(tmp.path()), "{}", inner.display());
    let name = inner.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!((name.len(), &name[..2]), (8, "c."), "{name}");
    assert!(!inner.exists(), "{} is removed", inner.display());
    assert_eq!(entries(tmp.path()), Vec::<String>::new());
}

/// A command that leaves a directory in its TMPDIR fails, and the failure names the directory; the
/// guard removes it all the same.
#[test]
fn a_command_that_leaves_a_directory_fails_naming_it() {
    let tmp = TempDir::new().unwrap();
    let output = guarded(
        tmp.path(),
        r#"mkdir -p "$TMPDIR/.tmpLeft/inside" "$TMPDIR/cupel-mutate/0123456789abcdef""#,
    );
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("left 2 entries"), "{stderr}");
    assert!(stderr.contains(".tmpLeft\n"), "{stderr}");
    assert!(
        stderr.contains("cupel-mutate/0123456789abcdef\n"),
        "{stderr}"
    );
    assert!(!stderr.contains("inside"), "{stderr}");
    assert_eq!(entries(tmp.path()), Vec::<String>::new());
}

/// The mutation slots' root, `cupel-mutate`, is the machine's and stays once made; left empty, it is not
/// a leak.
#[test]
fn an_empty_mutation_slot_root_passes() {
    let tmp = TempDir::new().unwrap();
    let output = guarded(tmp.path(), r#"mkdir "$TMPDIR/cupel-mutate""#);
    assert_eq!(
        (output.status.code(), text(&output.stderr).as_str()),
        (Some(0), "")
    );
    assert_eq!(entries(tmp.path()), Vec::<String>::new());
}

/// With `--slots-of REPO`, the slots the mutation engine keeps for REPO in the caller's temporary
/// directory are in the command's for it to use, and are back in the caller's afterwards with what
/// the command added; they are not a leak.
#[test]
fn the_named_repositorys_slots_are_lent_to_the_command_and_kept() {
    let tmp = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    let kept = slots(tmp.path(), "aaaaaaaaaaaaaaaa", repo.path());
    let output = guarded_keeping(
        tmp.path(),
        repo.path(),
        r#"slot=$TMPDIR/cupel-mutate/aaaaaaaaaaaaaaaa/rust/0 && cat "$slot/built" && echo again > "$slot/again""#,
    );
    assert_eq!(
        (
            output.status.code(),
            text(&output.stdout),
            text(&output.stderr)
        ),
        (Some(0), "built\n".to_owned(), String::new())
    );
    assert_eq!(entries(&kept.join("rust/0")), ["again", "built"]);
    assert_eq!(entries(tmp.path()), ["cupel-mutate"]);
}

/// Slots the command makes for REPO, on a first run, are kept in the caller's temporary directory.
#[test]
fn slots_the_command_makes_for_the_named_repository_are_kept() {
    let tmp = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    let output = guarded_keeping(
        tmp.path(),
        repo.path(),
        r#"dir=$TMPDIR/cupel-mutate/bbbbbbbbbbbbbbbb && mkdir -p "$dir/rust/0" && printf '%s' "$1" > "$dir/repository""#,
    );
    assert_eq!(
        (output.status.code(), text(&output.stderr)),
        (Some(0), String::new())
    );
    assert_eq!(
        entries(&tmp.path().join("cupel-mutate/bbbbbbbbbbbbbbbb")),
        ["repository", "rust"]
    );
}

/// Slots of any other repository the command makes are still a leak, named and removed.
#[test]
fn another_repositorys_slots_are_still_a_leak() {
    let tmp = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    let other = TempDir::new().unwrap();
    let script = format!(
        r#"dir=$TMPDIR/cupel-mutate/cccccccccccccccc && mkdir -p "$dir" && printf '%s' '{}' > "$dir/repository""#,
        other.path().canonicalize().unwrap().display()
    );
    let output = guarded_keeping(tmp.path(), repo.path(), &script);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.ends_with(
            "left 1 entries in its temporary directory:\ncupel-mutate/cccccccccccccccc\n"
        ),
        "{stderr}"
    );
    assert_eq!(entries(tmp.path()), Vec::<String>::new());
}

/// A command that fails returns its own exit code, whatever it left.
#[test]
fn a_failing_command_returns_its_exit_code() {
    let tmp = TempDir::new().unwrap();
    let output = guarded(tmp.path(), r#"mkdir "$TMPDIR/.tmpLeft"; exit 4"#);
    assert_eq!(output.status.code(), Some(4), "{}", text(&output.stderr));
    assert_eq!(entries(tmp.path()), Vec::<String>::new());
}

/// `make parity` builds and runs the fixture suite under the guard, as `make check` runs its tests.
#[test]
fn make_parity_runs_under_the_guard() {
    assert_eq!(
        recipe("parity"),
        [
            "scripts/tmpdir-guard.sh $(CARGO) build --package cupel-cli",
            "DAEMON_PARITY=1 MCP_PARITY=1 scripts/tmpdir-guard.sh $(CARGO) test --package cupel-fixtures",
        ]
    );
}

/// `make check` runs its lint, tests, and docs under the guard.
#[test]
fn make_check_runs_its_tests_under_the_guard() {
    let check = recipe("check");
    let guarded: Vec<&String> = check
        .iter()
        .filter(|line| line.contains("lint test doc"))
        .collect();
    assert_eq!(
        guarded,
        ["@scripts/tmpdir-guard.sh $(MAKE) --no-print-directory lint test doc"],
        "{check:#?}"
    );
}
