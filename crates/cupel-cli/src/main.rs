// ABOUTME: Entry point of the `cupel` binary.
// ABOUTME: Parses the command line and maps the outcome to a process exit code.
use clap::Parser;

/// Cupel grades what a coding agent produced.
#[derive(Debug, Parser)]
#[command(name = "cupel", version, arg_required_else_help = true)]
struct Cli {}

fn main() {
    Cli::parse();
}
