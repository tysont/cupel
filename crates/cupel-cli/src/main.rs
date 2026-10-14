// ABOUTME: Entry point of the `cupel` binary.
// ABOUTME: Reads the process environment once, runs the invocation, and writes its output and exit code.
mod against;
mod app;
mod baseline;
mod catalog;
mod check;
mod cli;
mod describe;
mod doctor;
mod error;
mod events;
mod explain;
mod format;
mod init;
mod page;
mod persist;
mod problem;
mod project;
mod registry;
mod render;
mod run;
mod suppress;

use std::io::Write;

fn main() -> std::process::ExitCode {
    app::capture_panics();
    let output = app::run(&app::Environment::from_process());
    // A closed pipe on either stream is not an error worth reporting.
    let _ = std::io::stdout().write_all(output.stdout.as_bytes());
    let _ = std::io::stderr().write_all(output.stderr.as_bytes());
    std::process::ExitCode::from(output.code)
}
