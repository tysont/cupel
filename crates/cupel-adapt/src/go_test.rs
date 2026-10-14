// ABOUTME: The `go test -json` runner producer: the root's packages (`./...` by default) run with test2json
// ABOUTME: events on standard output and build errors on standard error, read by the test2json parser.
use cupel_core::{LangId, Langs, TestResult};

use crate::report::{listing, test2json};
use crate::runner::{Lister, Ran, Root, Runner, RunnerSpec, tests_flag};

const ID: &str = "tool.go-test";

pub(crate) static GO_TEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("go")]),
    tool: "go",
    node_bin: false,
    version: &["go", "version"],
    command,
    env: &[],
    merged: false,
    read,
    select: |selected| tests_flag(selected, "-run", run_pattern),
    list: Some(Lister {
        command: list_command,
        parse: listing::go,
    }),
    completed_codes: &[0],
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_runner(Runner(&GO_TEST)),
});

fn command(root: &Root) -> Vec<String> {
    [root.program.clone(), "test".to_owned(), "-json".to_owned()]
        .into_iter()
        .chain(packages(&root.packages))
        .collect()
}

/// A root's package patterns, `./...` when it names none.
pub(crate) fn packages(patterns: &[String]) -> Vec<String> {
    if patterns.is_empty() {
        vec!["./...".to_owned()]
    } else {
        patterns.to_vec()
    }
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "every runner reads through `RunnerSpec::read`; this format always yields a list"
)]
fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    Some(test2json::parse(&ran.captured.stdout, &ran.captured.stderr))
}

/// The `-run` pattern matching exactly the top-level tests of `ids`.
pub(crate) fn run_pattern(ids: &[String]) -> String {
    let mut names: Vec<&str> = ids.iter().filter_map(|id| id.rsplit('/').next()).collect();
    names.sort_unstable();
    names.dedup();
    format!("^({})$", names.join("|"))
}

fn list_command(root: &Root) -> Vec<String> {
    [
        root.program.clone(),
        "test".to_owned(),
        "-list".to_owned(),
        "^Test".to_owned(),
        "-json".to_owned(),
    ]
    .into_iter()
    .chain(packages(&root.packages))
    .collect()
}
