// ABOUTME: Tests for scripts/tmpdir-guard.sh, which `make check` runs its lint, tests, and docs under: a temporary
// ABOUTME: directory of their own as TMPDIR, and a failure naming whatever they left in it.
mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use support::{recipe, workspace};
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

/// The names of what `dir` holds.
fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_script_passes_shellcheck() {
    if cupel_core::find_program("shellcheck").is_none() {
        eprintln!("SKIPPED: shellcheck not installed; this test did not run");
        return;
    }
    let output = Command::new("shellcheck")
        .args(["--shell=sh", "scripts/tmpdir-guard.sh"])
        .current_dir(workspace())
        .output()
        .unwrap();
    assert_eq!(
        (output.status.code(), text(&output.stdout).as_str()),
        (Some(0), "")
    );
}

/// The command runs with a fresh directory under the caller's as its TMPDIR, which is gone after it.
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

/// A command that fails returns its own exit code, whatever it left.
#[test]
fn a_failing_command_returns_its_exit_code() {
    let tmp = TempDir::new().unwrap();
    let output = guarded(tmp.path(), r#"mkdir "$TMPDIR/.tmpLeft"; exit 4"#);
    assert_eq!(output.status.code(), Some(4), "{}", text(&output.stderr));
    assert_eq!(entries(tmp.path()), Vec::<String>::new());
}

/// `make check` runs its lint, tests, and docs under the guard.
#[test]
fn make_check_runs_its_tests_under_the_guard() {
    assert!(
        recipe("check").contains(
            &"@scripts/tmpdir-guard.sh $(MAKE) --no-print-directory lint test doc".to_owned()
        ),
        "{:#?}",
        recipe("check")
    );
}
