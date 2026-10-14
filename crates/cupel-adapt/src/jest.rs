// ABOUTME: The `jest` runner producer: `jest --ci --json` writing its report to `.cupel/`, from the root's
// ABOUTME: `node_modules/.bin` or `PATH`; read by the Jest JSON parser.
use cupel_core::{LangFamily, Langs, TestResult};

use crate::report::jest_json;
use crate::runner::{Ran, Root, Runner, RunnerSpec, Selected, jest_report_command};

const ID: &str = "tool.jest";

static JEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Family(LangFamily::from("ecma")),
    tool: "jest",
    node_bin: true,
    version: &["jest", "--version"],
    command,
    env: &[],
    merged: false,
    read,
    completed_codes: &[0],
    select: files,
    list: None,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_runner(Runner(&JEST)),
});

fn command(root: &Root) -> Vec<String> {
    jest_report_command(root, &["--ci", "--json"])
}

fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    jest_json::parse(ran.report.as_deref()?, &ran.root.dir)
}

/// Test files, appended as paths; tests by ID are not selectable.
fn files(selected: &Selected<'_>) -> Option<Vec<String>> {
    match selected {
        Selected::Files(files) => Some(files.to_vec()),
        Selected::Tests(_) => None,
    }
}
