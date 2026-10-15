// ABOUTME: The `pytest` runner producer: a JUnit report (`xunit1`, which names each test's file) written to `.cupel/`,
// ABOUTME: no cache directory and no bytecode written into the tree; read by the JUnit parser.
use cupel_core::{LangId, Langs, TestResult};

use crate::report::{junit, listing};
use crate::runner::{Lister, PYTHON, Ran, Root, Runner, RunnerSpec, Selected};

const ID: &str = "tool.pytest";

static PYTEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("python")]),
    tool: "pytest",
    node_bin: false,
    version: &["pytest", "--version"],
    command,
    env: &[("PYTHONDONTWRITEBYTECODE", "1")],
    merged: false,
    read,
    select,
    list: Some(Lister {
        command: collect,
        parse: listing::pytest,
    }),
    completed_codes: &[0, 5],
    vary: &PYTHON,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_runner(Runner(&PYTEST)),
});

fn command(root: &Root) -> Vec<String> {
    let mut command: Vec<String> = [
        root.program.as_str(),
        "-p",
        "no:cacheprovider",
        "-o",
        "junit_family=xunit1",
        "-o",
        "junit_logging=all",
    ]
    .map(str::to_owned)
    .to_vec();
    command.push(format!("--junitxml={}", root.report.display()));
    command
}

fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    junit::pytest(ran.report.as_deref()?)
}

/// Node IDs and test files are both arguments pytest runs.
#[allow(
    clippy::unnecessary_wraps,
    reason = "every runner narrows through `RunnerSpec::select`; pytest narrows to anything"
)]
fn select(selected: &Selected<'_>) -> Option<Vec<String>> {
    match selected {
        Selected::Tests(paths) | Selected::Files(paths) => Some(paths.to_vec()),
    }
}

/// Collects the suite without running it, one node ID per line.
fn collect(root: &Root) -> Vec<String> {
    [
        root.program.as_str(),
        "-p",
        "no:cacheprovider",
        "--collect-only",
        "-q",
    ]
    .map(str::to_owned)
    .to_vec()
}
