// ABOUTME: The `jest` runner producer: `jest --ci --json` writing its report to `.cupel/`, from the root's
// ABOUTME: `node_modules/.bin` or `PATH`; read by the Jest JSON parser.
use cupel_core::{LangFamily, Langs, TestResult, TestResults};

use crate::report::jest_json;
use crate::runner::{Ran, Root, Runner, RunnerSpec, jest_report_command};

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
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<TestResults, _>(Runner(&JEST)),
});

fn command(root: &Root) -> Vec<String> {
    jest_report_command(root, &["--ci", "--json"])
}

fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    jest_json::parse(ran.report.as_deref()?, &ran.root.dir)
}
