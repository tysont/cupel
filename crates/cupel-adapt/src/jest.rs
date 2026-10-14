// ABOUTME: The `jest` runner producer: `jest --ci --json` writing its report to `.cupel/`, from the root's
// ABOUTME: `node_modules/.bin` or `PATH`; read by the Jest JSON parser.
use cupel_core::{LangFamily, Langs, Process, TestResult, TestResults};

use crate::report::jest_json;
use crate::runner::{Ran, Root, Runner, RunnerSpec};

const ID: &str = "tool.jest";

static JEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Family(LangFamily::from("ecma")),
    tool: "jest",
    node_bin: true,
    version: &["jest", "--version"],
    command,
    read,
    completed_codes: &[0],
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<TestResults, _>(Runner(&JEST)),
});

fn command(root: &Root) -> Process {
    Process::new(&root.program, &root.dir).args([
        "--ci".to_owned(),
        "--json".to_owned(),
        "--passWithNoTests".to_owned(),
        format!("--outputFile={}", root.report.display()),
    ])
}

fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    jest_json::parse(ran.report.as_deref()?, &ran.root.dir)
}
