// ABOUTME: A test body run in a child of its own test binary whose TMPDIR the call owns and removes, for tests whose
// ABOUTME: in-process producers start tools that keep files there (jest's cache, node's compile cache).
//! A test that runs a producer in its own process cannot give the tools the producer starts a
//! temporary directory of their own: they inherit the test process's `TMPDIR`, and changing a
//! process's environment while its other tests run is unsound. [`in_own_tmpdir`] runs the test
//! again, alone, in a child of its test binary with `TMPDIR` set to a directory the call owns, so
//! whatever the tools keep there goes when the call ends.

use std::process::Command;

/// The variable that marks the child.
const CHILD: &str = "CUPEL_FIXTURES_OWN_TMPDIR";

/// Whether this process is the child [`in_own_tmpdir`] started.
pub fn is_child() -> bool {
    std::env::var_os(CHILD).is_some()
}

/// Runs `body`, the body of the test named `test` (its full path within the binary), in a child of
/// this test binary that runs that test alone, with `TMPDIR` a directory removed when the child
/// ends; in the child, runs `body` itself. A child that fails fails the test with its output; one
/// that passes passes on its standard error (a skipped test's reason), not the harness's report.
///
/// # Panics
///
/// When the child cannot start, its test fails, or it ran no test named `test`.
pub fn in_own_tmpdir(test: &str, body: impl FnOnce()) {
    if is_child() {
        body();
        return;
    }
    let tmp = tempfile::tempdir().expect("a temporary directory for the child");
    let output = Command::new(std::env::current_exe().expect("the test binary's path"))
        .args([test, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .env("TMPDIR", tmp.path())
        .output()
        .expect("the test binary runs its test again");
    drop(tmp);
    let (stdout, stderr) = (
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        output.status.success(),
        "{test} failed in its own temporary directory:\n{stdout}{stderr}"
    );
    // Both libtest and libtest-mimic end with `test result: ok. 1 passed; ...` for one test run.
    assert!(
        stdout.contains(" 1 passed;"),
        "the child ran no test named {test}:\n{stdout}"
    );
    eprint!("{stderr}");
}
