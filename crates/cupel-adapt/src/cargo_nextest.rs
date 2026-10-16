// ABOUTME: The `cargo nextest` runner producer: `cargo nextest run` with a tool configuration file whose `cupel`
// ABOUTME: profile writes a JUnit report under the target directory, read by the JUnit parser.
use cupel_core::{LangId, Langs, TestResult};

use crate::report::{junit, listing};
use crate::runner::{FIXED, Lister, Ran, Root, Runner, RunnerSpec, tests_flag};

const ID: &str = "tool.cargo-nextest";

pub(crate) static CARGO_NEXTEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("rust")]),
    tool: "cargo-nextest",
    node_bin: false,
    version: &["cargo", "nextest", "--version"],
    command,
    // nextest hands its profile to the tests it runs as `NEXTEST_PROFILE`, which a nextest run
    // inside one of them reads; `run` names its own, and `list` takes the default.
    env: &[
        ("CARGO_TERM_COLOR", "never"),
        ("NEXTEST_PROFILE", "default"),
    ],
    merged: false,
    read,
    select: |selected| tests_flag(selected, "-E", filterset),
    list: Some(Lister {
        command: list_command,
        parse: listing::nextest,
    }),
    completed_codes: &[0],
    vary: &FIXED,
    outputs: &["target"],
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_runner(Runner(&CARGO_NEXTEST)),
});

/// The `cupel` profile: a `JUnit` report that keeps the output of failures.
pub(crate) const PROFILE: &str = "[profile.cupel.junit]\npath = \"junit.xml\"\nstore-success-output = false\nstore-failure-output = true\n";

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

/// The filterset that matches exactly the nextest tests `ids` (`<binary-id> <test path>`).
pub(crate) fn filterset(ids: &[String]) -> String {
    ids.iter()
        .map(|id| match id.split_once(' ') {
            Some((binary, test)) => format!("(binary_id(={binary}) & test(={test}))"),
            None => format!("test(={id})"),
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn list_command(_: &Root) -> Vec<String> {
    [
        "cargo",
        "nextest",
        "list",
        "--workspace",
        "--message-format",
        "json",
    ]
    .map(str::to_owned)
    .to_vec()
}
