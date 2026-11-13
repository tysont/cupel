// ABOUTME: Tests for the daemon-parity harness itself (ADR-085 §11): a run through the daemon on a copy matches the
// ABOUTME: cold run, as does an MCP tool call (ADR-121), and a result that differs is caught, so parity is not vacuous.
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::time::{Duration, SystemTime};

use cupel_fixtures::parity::Twin;
use cupel_fixtures::run_cupel;

const ARGS: [&str; 3] = ["check", "--format", "json"];

#[test]
fn a_daemon_run_on_a_copy_matches_the_cold_run_and_a_difference_is_caught() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.py"),
        "def a(x):\n    return x if x else 0\n",
    )
    .unwrap();
    let twin = Twin::of(dir.path());
    let cold = run_cupel(&ARGS, dir.path());
    assert_eq!(cold.code, Some(0), "{}", cold.stdout);
    twin.assert_same(&ARGS, (cold.code, &cold.stdout, &cold.stderr));

    let other = Twin::of(dir.path());
    let caught = panic::catch_unwind(AssertUnwindSafe(|| {
        other.assert_same(&ARGS, (cold.code, &cold.stdout, "cupel: a line\n"));
    }));
    let message = caught
        .expect_err("a different stderr must fail parity")
        .downcast::<String>()
        .unwrap();
    assert!(
        message.contains("differs through the daemon") && message.contains("-cupel: a line"),
        "{message}"
    );
}

/// A copy keeps each file's modification time: freshness trusts a file not modified since its
/// record without rehashing it (ADR-079 §4), so a copy dated in the order it was made would
/// trust a source edited after the record whenever the record was copied later.
#[test]
fn a_copy_keeps_each_files_modification_time() {
    let dir = tempfile::tempdir().unwrap();
    let edited = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000);
    let recorded = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    dated(&dir.path().join("a/edited.txt"), edited);
    dated(&dir.path().join("z/sources.json"), recorded);
    let twin = Twin::of(dir.path());
    assert_eq!(modified(&twin.root().join("a/edited.txt")), edited);
    assert_eq!(modified(&twin.root().join("z/sources.json")), recorded);
}

/// Writes a read-only file at `path`, as git's objects are, modified at `at`.
fn dated(path: &Path, at: SystemTime) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "text\n").unwrap();
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(at).unwrap();
    let mut permissions = file.metadata().unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions).unwrap();
}

fn modified(path: &Path) -> SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

#[test]
fn an_mcp_call_on_a_copy_matches_the_cold_run_and_a_difference_is_caught() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.py"),
        "def a(x):\n    return x if x else 0\n",
    )
    .unwrap();
    let twin = Twin::of(dir.path());
    let cold = run_cupel(&ARGS, dir.path());
    let ran = (cold.code, cold.stdout.as_str(), cold.stderr.as_str());
    assert!(
        twin.assert_same_through_mcp("check", &ARGS, ran),
        "compared"
    );

    let other = Twin::of(dir.path());
    let caught = panic::catch_unwind(AssertUnwindSafe(|| {
        other.assert_same_through_mcp("check", &ARGS, (cold.code, &cold.stdout, "cupel: a line\n"));
    }));
    let message = caught
        .expect_err("a different stderr must fail parity")
        .downcast::<String>()
        .unwrap();
    assert!(
        message.contains("differs through serve --mcp") && message.contains("-cupel: a line"),
        "{message}"
    );

    // A flag no tool takes, and a command line without the `--format json` a call adds, are not
    // compared; `--format json` anywhere is.
    let withheld = ["check", "--no-color", "--format", "json"];
    let unformatted = ["check", "--only", "static.complexity"];
    for args in [withheld.as_slice(), unformatted.as_slice()] {
        assert!(!Twin::of(dir.path()).assert_same_through_mcp("check", args, ran));
    }
    let formatted_first = ["check", "--format", "json", "--only", "static.complexity"];
    let cold = run_cupel(&formatted_first, dir.path());
    let ran = (cold.code, cold.stdout.as_str(), cold.stderr.as_str());
    assert!(Twin::of(dir.path()).assert_same_through_mcp("check", &formatted_first, ran));
}
