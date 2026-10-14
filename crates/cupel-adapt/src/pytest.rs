// ABOUTME: The `pytest` runner producer: a JUnit report (`xunit1`, which names each test's file) written to `.cupel/`,
// ABOUTME: no cache directory and no bytecode written into the tree; read by the JUnit parser.
use cupel_core::{LangId, Langs, Process, TestResult, TestResults};

use crate::report::junit;
use crate::runner::{Ran, Root, Runner, RunnerSpec};

const ID: &str = "tool.pytest";

static PYTEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("python")]),
    tool: "pytest",
    node_bin: false,
    version: &["pytest", "--version"],
    command,
    read,
    completed_codes: &[0, 5],
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<TestResults, _>(Runner(&PYTEST)),
});

fn command(root: &Root) -> Process {
    Process::new(&root.program, &root.dir)
        .args([
            "-p",
            "no:cacheprovider",
            "-o",
            "junit_family=xunit1",
            "-o",
            "junit_logging=all",
        ])
        .args([format!("--junitxml={}", root.report.display())])
        .env("PYTHONDONTWRITEBYTECODE", "1")
}

fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    junit::pytest(ran.report.as_deref()?)
}
