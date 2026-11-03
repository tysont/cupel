// ABOUTME: Tests for scripts/tmpdir-guard.sh, which `make check`'s suites, `make parity`, and `make dogfood`'s gate run under:
// ABOUTME: a short temporary directory of their own under /tmp as TMPDIR, and a failure naming whatever they left in it.
#![cfg(unix)]
mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use support::{assert_executable, assert_shellcheck_clean, entries, recipe, workspace};
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

/// `make check` runs the script directly, so it is executable; and it passes `shellcheck`.
#[test]
fn the_script_is_executable_and_passes_shellcheck() {
    assert_executable("scripts/tmpdir-guard.sh");
    assert_shellcheck_clean("scripts/tmpdir-guard.sh");
}

/// The command runs with a fresh directory under `/tmp` as its TMPDIR, whatever the caller's, which is gone after
/// it. Its path is short: a guarded mutation run nests its own TMPDIR and a test's repository below it, and the
/// socket a daemon test makes in that repository must fit macOS's 103-character socket path with room to spare
/// (`/private/tmp/` 13, this 9, the run's 7, the repository's 11, `.cupel/serve.sock` 17: 57).
#[test]
fn a_command_that_leaves_nothing_passes_and_its_temporary_directory_goes() {
    let tmp = TempDir::new().unwrap();
    let output = guarded(
        tmp.path(),
        r#"made=$(mktemp -d) && rmdir "$made" && printf '%s' "$TMPDIR""#,
    );
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let inner = PathBuf::from(text(&output.stdout));
    assert_eq!(
        inner.parent(),
        Some(Path::new("/tmp")),
        "{}",
        inner.display()
    );
    let name = inner.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!((name.len(), &name[..2]), (8, "c."), "{name}");
    assert!(!inner.exists(), "{} is removed", inner.display());
    assert_eq!(entries(tmp.path()), Vec::<String>::new());
}

/// A command that leaves anything in its TMPDIR fails, and the failure names each top-level entry it left; the
/// guard removes them all the same.
#[test]
fn a_command_that_leaves_a_directory_fails_naming_it() {
    let tmp = TempDir::new().unwrap();
    let output = guarded(
        tmp.path(),
        r#"mkdir -p "$TMPDIR/.tmpLeft/inside" "$TMPDIR/cupel-mutate/0123456789abcdef" && printf '%s' "$TMPDIR""#,
    );
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.ends_with("left 2 entries in its temporary directory:\n.tmpLeft\ncupel-mutate\n"),
        "{stderr}"
    );
    assert!(!Path::new(&text(&output.stdout)).exists());
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
