// ABOUTME: Tests for the daemon-parity harness itself (ADR-085 §11): a run through the daemon on a copy matches the
// ABOUTME: cold run, and a cold result that differs is caught with a diff, so `make parity` cannot pass vacuously.
use std::panic::{self, AssertUnwindSafe};

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
