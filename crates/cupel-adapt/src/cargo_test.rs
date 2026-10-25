// ABOUTME: The `cargo test` runner producer: `cargo test --workspace --no-fail-fast`, standard error merged into
// ABOUTME: standard output so each test follows its target's `Running` line, read by the libtest text parser.
use cupel_core::{LangId, Langs, RunOptions, RunOrder, RunOrdering, TestResult};

use crate::report::libtest;
use crate::runner::{
    Executed, Ran, Root, Runner, RunnerSpec, Varying, keeps_every_test, whole_suite,
};

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
    exclude: keeps_every_test,
    list: None,
    vary: &Varying {
        ordering: RunOrdering::Seeded,
        args: vary,
        env: |_| Vec::new(),
        executed: Executed::Report,
    },
    outputs: &["target"],
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

/// `--test-threads`, and for a shuffle libtest's `--shuffle-seed` over the test targets (no
/// doc-tests, whose harness takes no unstable flags): unstable on stable Rust, so the test
/// binaries (never `rustc`) run with `RUSTC_BOOTSTRAP=1` through a target runner, which
/// rebuilds nothing (ADR-078).
fn vary(options: &RunOptions) -> Vec<String> {
    let mut args = Vec::new();
    let mut harness = Vec::new();
    if options.order == RunOrder::Shuffled {
        args.extend(
            [
                "--tests",
                "--config",
                r#"target.'cfg(all())'.runner = ["env", "RUSTC_BOOTSTRAP=1"]"#,
            ]
            .map(str::to_owned),
        );
        harness.extend(["-Z", "unstable-options", "--shuffle-seed"].map(str::to_owned));
        harness.push(options.seed.unwrap_or_default().to_string());
    }
    if let Some(threads) = options.parallelism {
        harness.push(format!("--test-threads={threads}"));
    }
    if !harness.is_empty() {
        args.push("--".to_owned());
        args.extend(harness);
    }
    args
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "every runner reads through `RunnerSpec::read`; this format always yields a list"
)]
fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    Some(libtest::parse(&ran.captured.stdout))
}
