// ABOUTME: The `cargo test` runner producer: `cargo test --workspace --no-fail-fast`, standard error merged into
// ABOUTME: standard output so each test follows its target's `Running` line, read by the libtest text parser.
use cupel_core::{LangId, Langs, Process, TestResult, TestResults};

use crate::report::libtest;
use crate::runner::{Ran, Root, Runner, RunnerSpec};

const ID: &str = "tool.cargo-test";

static CARGO_TEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("rust")]),
    tool: "cargo",
    node_bin: false,
    version: &["cargo", "--version"],
    command,
    read,
    completed_codes: &[0],
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<TestResults, _>(Runner(&CARGO_TEST)),
});

fn command(root: &Root) -> Process {
    Process::new(&root.program, &root.dir)
        .args(["test", "--workspace", "--no-fail-fast"])
        .env("CARGO_TERM_COLOR", "never")
        .merged()
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "every runner reads through `RunnerSpec::read`; this format always yields a list"
)]
fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    Some(libtest::parse(&ran.captured.stdout))
}
