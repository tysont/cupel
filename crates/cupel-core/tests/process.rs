// ABOUTME: Tests for `Process`, the one way producers run programs (ADR-076 §3): captured output and exit
// ABOUTME: codes, merged streams in order, environment and directory, cancellation, and spawn failures.
#![cfg(unix)]

use std::time::{Duration, Instant};

use cupel_core::{Cancellation, Captured, Error, Process, find_program};

fn shell(script: &str) -> Process {
    Process::new("sh", std::env::temp_dir()).args(["-c", script])
}

#[test]
fn output_and_exit_code_are_captured_per_stream() {
    let captured = shell("echo out; echo err >&2; exit 3")
        .run(&Cancellation::new())
        .unwrap();
    assert_eq!(
        captured,
        Captured {
            exit_code: Some(3),
            stdout: "out\n".to_owned(),
            stderr: "err\n".to_owned(),
        }
    );
}

#[test]
fn merged_output_keeps_the_order_it_was_written_in() {
    let captured = shell("echo one; echo two >&2; echo three")
        .merged()
        .run(&Cancellation::new())
        .unwrap();
    assert_eq!(
        (captured.stdout.as_str(), captured.stderr.as_str()),
        ("one\ntwo\nthree\n", "")
    );
}

#[test]
fn the_child_runs_in_its_directory_with_the_extra_environment() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().canonicalize().unwrap();
    let captured = Process::new("sh", &real)
        .args(["-c", "echo \"$CUPEL_PROBE\"; pwd -P"])
        .env("CUPEL_PROBE", "seen")
        .run(&Cancellation::new())
        .unwrap();
    assert_eq!(captured.stdout, format!("seen\n{}\n", real.display()));
}

#[test]
fn cancellation_kills_the_child_and_its_process_group() {
    let started = Instant::now();
    let cancel = Cancellation::with_deadline(Instant::now() + Duration::from_millis(200));
    let ran = shell("sleep 30 & sleep 30").run(&cancel);
    assert_eq!(ran, Err(Error::Cancelled));
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "cancelled after {:?}",
        started.elapsed()
    );
}

#[test]
fn a_cancelled_token_runs_nothing() {
    let cancel = Cancellation::new();
    cancel.cancel();
    assert_eq!(shell("exit 0").run(&cancel), Err(Error::Cancelled));
}

#[test]
fn a_program_that_cannot_start_is_a_process_error() {
    let error = Process::new("cupel-no-such-program", std::env::temp_dir())
        .run(&Cancellation::new())
        .unwrap_err()
        .to_string();
    assert_eq!(
        error.split(": ").next(),
        Some("cannot run `cupel-no-such-program`")
    );
}

#[test]
fn the_command_line_is_the_program_then_its_arguments() {
    assert_eq!(
        shell("true").command_line(),
        ["sh".to_owned(), "-c".to_owned(), "true".to_owned()]
    );
}

#[test]
fn programs_are_found_on_the_path() {
    assert_eq!(
        (
            find_program("sh").is_some(),
            find_program("cupel-no-such-program")
        ),
        (true, None)
    );
}

/// Re-runs this test binary on one test with `vars` set, so the test can check what a child of
/// `Process` inherits without changing this process's environment.
fn in_environment(test: &str, vars: &[(&str, &str)]) -> std::process::Output {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([test, "--exact", "--nocapture"])
        .env("CUPEL_PROCESS_TEST_CHILD", "1")
        // A harness around this test run must not decide the test.
        .env_remove("__CARGO_LLVM_COV_RUSTC_WRAPPER");
    for (key, value) in vars {
        command.env(key, value);
    }
    command.output().unwrap()
}

/// A coverage harness measuring Cupel itself (cargo-llvm-cov's rustc wrapper and profile file)
/// must not reach the tools Cupel runs: the fixture a producer builds is not Cupel's workspace.
#[test]
fn a_coverage_harness_around_cupel_does_not_reach_the_child() {
    if std::env::var_os("CUPEL_PROCESS_TEST_CHILD").is_some() {
        let captured = shell("env").run(&Cancellation::new()).unwrap();
        let leaked: Vec<&str> = captured
            .stdout
            .lines()
            .filter(|line| {
                line.starts_with("CARGO_LLVM_COV")
                    || line.starts_with("__CARGO_LLVM_COV_")
                    || line.starts_with("LLVM_PROFILE_FILE=")
                    || line.starts_with("RUSTC_WRAPPER=")
                    || line.starts_with("RUSTC_WORKSPACE_WRAPPER=")
            })
            .collect();
        assert_eq!(leaked, Vec::<&str>::new());
        assert!(captured.stdout.contains("KEPT=yes"));
        return;
    }
    let output = in_environment(
        "a_coverage_harness_around_cupel_does_not_reach_the_child",
        &[
            ("CARGO_LLVM_COV", "1"),
            ("CARGO_LLVM_COV_TARGET_DIR", "/t"),
            ("__CARGO_LLVM_COV_RUSTC_WRAPPER", "1"),
            ("RUSTC_WRAPPER", "/bin/cargo-llvm-cov"),
            ("RUSTC_WORKSPACE_WRAPPER", "/bin/cargo-llvm-cov"),
            ("LLVM_PROFILE_FILE", "/t/%m.profraw"),
            ("KEPT", "yes"),
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A rustc wrapper that is not cargo-llvm-cov's is the user's own and stays.
#[test]
fn a_rustc_wrapper_of_the_users_own_is_inherited() {
    if std::env::var_os("CUPEL_PROCESS_TEST_CHILD").is_some() {
        let captured = shell("env").run(&Cancellation::new()).unwrap();
        let wrappers: Vec<&str> = captured
            .stdout
            .lines()
            .filter(|line| line.starts_with("RUSTC_WRAPPER="))
            .collect();
        assert_eq!(wrappers, vec!["RUSTC_WRAPPER=/bin/sccache"]);
        return;
    }
    let output = in_environment(
        "a_rustc_wrapper_of_the_users_own_is_inherited",
        &[("RUSTC_WRAPPER", "/bin/sccache")],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}
