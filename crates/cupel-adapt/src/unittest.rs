// ABOUTME: The `unittest` runner producer: `python3 -m unittest discover -v -b`, output merged so each failure's
// ABOUTME: buffered output follows it, no bytecode written into the tree; read by the unittest text parser.
use cupel_core::{LangId, Langs, TestResult};

use crate::report::unittest;
use crate::runner::{PYTHON, Ran, Root, Runner, RunnerSpec, as_arguments, keeps_every_test};

const ID: &str = "tool.unittest";

static UNITTEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("python")]),
    tool: "python3",
    node_bin: false,
    version: &["python3", "--version"],
    command,
    env: &[("PYTHONDONTWRITEBYTECODE", "1")],
    merged: true,
    read,
    // 5: no tests ran (Python 3.12+), a finished run of an empty suite.
    completed_codes: &[0, 5],
    // Dotted test IDs are arguments unittest runs; files are not.
    select: |selected| as_arguments(selected, false),
    exclude: keeps_every_test,
    list: None,
    vary: &PYTHON,
    outputs: &[],
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_runner(Runner(&UNITTEST)),
});

/// Discovery, or (narrowed) the dotted test IDs that follow, run in the order given.
fn command(root: &Root) -> Vec<String> {
    let discover = (!root.narrowed).then_some("discover");
    [root.program.as_str(), "-m", "unittest"]
        .into_iter()
        .chain(discover)
        .chain(["-v", "-b"])
        .map(str::to_owned)
        .collect()
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "every runner reads through `RunnerSpec::read`; this format always yields a list"
)]
fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    Some(unittest::parse(&ran.captured.stdout))
}
