// ABOUTME: The `go test -json` runner producer: the root's packages (`./...` by default) run with test2json
// ABOUTME: events on standard output and build errors on standard error, read by the test2json parser.
use cupel_core::{LangId, Langs, Process, TestResult, TestResults};

use crate::report::test2json;
use crate::runner::{Ran, Root, Runner, RunnerSpec};

const ID: &str = "tool.go-test";

static GO_TEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("go")]),
    tool: "go",
    node_bin: false,
    version: &["go", "version"],
    command,
    read,
    completed_codes: &[0],
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<TestResults, _>(Runner(&GO_TEST)),
});

fn command(root: &Root) -> Process {
    let packages = if root.packages.is_empty() {
        vec!["./...".to_owned()]
    } else {
        root.packages.clone()
    };
    Process::new(&root.program, &root.dir)
        .args(["test", "-json"])
        .args(packages)
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "every runner reads through `RunnerSpec::read`; this format always yields a list"
)]
fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    Some(test2json::parse(&ran.captured.stdout, &ran.captured.stderr))
}
