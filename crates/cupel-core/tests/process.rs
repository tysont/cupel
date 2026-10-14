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
    let ran = Process::new("cupel-no-such-program", std::env::temp_dir()).run(&Cancellation::new());
    let program = match ran {
        Err(Error::Process { program, .. }) => program,
        other => panic!("expected a process error, got {other:?}"),
    };
    assert_eq!(program, "cupel-no-such-program");
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
