// ABOUTME: The `cargo test` runner producer: `cargo test --workspace --no-fail-fast`, standard error merged into
// ABOUTME: standard output so each test follows its target's `Running` line, read by the libtest text parser.
use cupel_core::{LangId, Langs, TestResult};

use crate::report::libtest;
use crate::runner::{Ran, Root, Runner, RunnerSpec, whole_suite};

const ID: &str = "tool.cargo-test";

static CARGO_TEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("rust")]),
    tool: "cargo",
    node_bin: false,
    version: &["cargo", "--version"],
    command,
    env: &[("CARGO_TERM_COLOR", "never")],
    merged: true,
    read,
    completed_codes: &[0],
    select: whole_suite,
    list: None,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_runner(Runner(&CARGO_TEST)),
});

fn command(root: &Root) -> Vec<String> {
    [
        root.program.as_str(),
        "test",
        "--workspace",
        "--no-fail-fast",
    ]
    .map(str::to_owned)
    .to_vec()
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "every runner reads through `RunnerSpec::read`; this format always yields a list"
)]
fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    Some(libtest::parse(&ran.captured.stdout))
}
