// ABOUTME: The `vitest` runner producer: `vitest run` with the JSON reporter writing a Jest-shaped report to
// ABOUTME: `.cupel/`, from the root's `node_modules/.bin` or `PATH`; read by the Jest JSON parser.
use cupel_core::{LangFamily, Langs, Process, TestResult, TestResults};

use crate::report::jest_json;
use crate::runner::{Ran, Root, Runner, RunnerSpec};

const ID: &str = "tool.vitest";

static VITEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Family(LangFamily::from("ecma")),
    tool: "vitest",
    node_bin: true,
    version: &["vitest", "--version"],
    command,
    read,
    completed_codes: &[0],
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<TestResults, _>(Runner(&VITEST)),
});

fn command(root: &Root) -> Process {
    Process::new(&root.program, &root.dir).args([
        "run".to_owned(),
        "--reporter=json".to_owned(),
        "--passWithNoTests".to_owned(),
        format!("--outputFile={}", root.report.display()),
    ])
}

fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    jest_json::parse(ran.report.as_deref()?, &ran.root.dir)
}
