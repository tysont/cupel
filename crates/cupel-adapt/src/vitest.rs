// ABOUTME: The `vitest` runner producer: `vitest run` with the JSON reporter writing a Jest-shaped report to
// ABOUTME: `.cupel/`, from the root's `node_modules/.bin` or `PATH`; read by the Jest JSON parser.
use cupel_core::{LangFamily, Langs, RunOrdering, TestResult};

use crate::report::jest_json;
use crate::runner::{
    Executed, Ran, Root, Runner, RunnerSpec, Varying, files, jest_report_command,
    jest_report_variation, keeps_every_test,
};

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
    exclude: keeps_every_test,
    list: None,
    vary: &Varying {
        ordering: RunOrdering::Seeded,
        args: |options| jest_report_variation(options, "--sequence.seed", "--sequence.shuffle"),
        env: |_| Vec::new(),
        executed: Executed::Unknown,
    },
    outputs: &[],
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
