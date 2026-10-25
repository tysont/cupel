// ABOUTME: The `jest` runner producer: `jest --ci --json` writing its report to `.cupel/`, from the root's
// ABOUTME: `node_modules/.bin` or `PATH`; read by the Jest JSON parser.
use cupel_core::{LangFamily, Langs, RunOrdering, TestResult};

use crate::report::jest_json;
use crate::runner::{
    Executed, Ran, Root, Runner, RunnerSpec, Varying, files, jest_report_command,
    jest_report_variation, keeps_every_test,
};

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
    exclude: keeps_every_test,
    list: None,
    vary: &Varying {
        ordering: RunOrdering::Seeded,
        args: |options| jest_report_variation(options, "--seed", "--randomize"),
        env: |_| Vec::new(),
        executed: Executed::Unknown,
    },
    outputs: &[],
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
