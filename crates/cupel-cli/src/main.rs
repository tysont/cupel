// ABOUTME: Entry point of the `cupel` binary.
// ABOUTME: Reads the process environment once, runs the invocation, and writes its output and exit code.
mod app;
mod cli;
mod describe;
mod detect;
mod doctor;
mod error;
mod events;
mod format;
mod git;
mod init;
mod page;
mod problem;
mod registry;
mod render;
mod run;

use std::io::Write;

fn main() -> std::process::ExitCode {
    app::capture_panics();
    let output = app::run(&app::Environment::from_process());
    // A closed pipe on either stream is not an error worth reporting.
    let _ = std::io::stdout().write_all(output.stdout.as_bytes());
    let _ = std::io::stderr().write_all(output.stderr.as_bytes());
    std::process::ExitCode::from(output.code)
}
