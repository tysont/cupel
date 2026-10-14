// ABOUTME: The `cargo nextest` runner producer: `cargo nextest run` with a tool configuration file whose `cupel`
// ABOUTME: profile writes a JUnit report under the target directory, read by the JUnit parser.
use cupel_core::{LangId, Langs, TestResult, TestResults};

use crate::report::junit;
use crate::runner::{Ran, Root, Runner, RunnerSpec};

const ID: &str = "tool.cargo-nextest";

static CARGO_NEXTEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("rust")]),
    tool: "cargo-nextest",
    node_bin: false,
    version: &["cargo", "nextest", "--version"],
    command,
    env: &[("CARGO_TERM_COLOR", "never")],
    merged: false,
    read,
    completed_codes: &[0],
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<TestResults, _>(Runner(&CARGO_NEXTEST)),
});

/// The `cupel` profile: a `JUnit` report that keeps the output of failures.
const PROFILE: &str = "[profile.cupel.junit]\npath = \"junit.xml\"\nstore-success-output = false\nstore-failure-output = true\n";

fn command(root: &Root) -> Vec<String> {
    // The profile file lives in `.cupel/`; a failure to write it surfaces as nextest's error.
    let _ = std::fs::write(&root.config, PROFILE);
    let mut command: Vec<String> = [
        "cargo",
        "nextest",
        "run",
        "--workspace",
        "--no-fail-fast",
        "--profile",
        "cupel",
        "--tool-config-file",
    ]
    .map(str::to_owned)
    .to_vec();
    command.push(format!("cupel:{}", root.config.display()));
    command
}

fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    let report = std::fs::read_to_string(
        ran.root
            .dir
            .join("target")
            .join("nextest")
            .join("cupel")
            .join("junit.xml"),
    )
    .ok()?;
    junit::nextest(&report)
}
