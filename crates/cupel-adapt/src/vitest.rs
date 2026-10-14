// ABOUTME: The `vitest` runner producer: `vitest run` with the JSON reporter writing a Jest-shaped report to
// ABOUTME: `.cupel/`, from the root's `node_modules/.bin` or `PATH`; read by the Jest JSON parser.
use cupel_core::{LangFamily, Langs, TestResult};

use crate::report::jest_json;
use crate::runner::{Ran, Root, Runner, RunnerSpec, files, jest_report_command};

const ID: &str = "tool.vitest";

static VITEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Family(LangFamily::from("ecma")),
    tool: "vitest",
    node_bin: true,
    version: &["vitest", "--version"],
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
    register: |registry| registry.register_runner(Runner(&VITEST)),
});

fn command(root: &Root) -> Vec<String> {
    jest_report_command(root, &["run", "--reporter=json"])
}

fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    jest_json::parse(ran.report.as_deref()?, &ran.root.dir)
}
