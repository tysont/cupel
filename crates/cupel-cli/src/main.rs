// ABOUTME: Entry point of the `cupel` binary.
// ABOUTME: Reads the process environment once, runs the invocation, and writes its output and exit code.
use std::io::Write;

fn main() -> std::process::ExitCode {
    cupel_cli::capture_panics();
    let output = cupel_cli::run(&cupel_cli::Environment::from_process());
    // A closed pipe on either stream is not an error worth reporting.
    let _ = std::io::stdout().write_all(output.stdout.as_bytes());
    let _ = std::io::stderr().write_all(output.stderr.as_bytes());
    std::process::ExitCode::from(output.code)
}
