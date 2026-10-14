// ABOUTME: The `unittest` runner producer: `python3 -m unittest discover -v -b`, output merged so each failure's
// ABOUTME: buffered output follows it, no bytecode written into the tree; read by the unittest text parser.
use cupel_core::{LangId, Langs, Process, TestResult, TestResults};

use crate::report::unittest;
use crate::runner::{Ran, Root, Runner, RunnerSpec};

const ID: &str = "tool.unittest";

static UNITTEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("python")]),
    tool: "python3",
    node_bin: false,
    version: &["python3", "--version"],
    command,
    read,
    completed_codes: &[0],
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<TestResults, _>(Runner(&UNITTEST)),
});

fn command(root: &Root) -> Process {
    Process::new(&root.program, &root.dir)
        .args(["-m", "unittest", "discover", "-v", "-b"])
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .merged()
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "every runner reads through `RunnerSpec::read`; this format always yields a list"
)]
fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    Some(unittest::parse(&ran.captured.stdout))
}
