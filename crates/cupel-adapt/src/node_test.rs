// ABOUTME: The `node --test` runner producer: Node's built-in test runner with its JUnit reporter writing to `.cupel/`,
// ABOUTME: read by the JUnit parser with IDs as Jest's; test files narrow it, and its concurrency varies it.
use cupel_core::{LangFamily, Langs, RunOptions, RunOrdering, TestResult};

use crate::report::junit;
use crate::runner::{Executed, Ran, Root, Runner, RunnerSpec, Varying, files, keeps_every_test};

const ID: &str = "tool.node-test";

static NODE_TEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Family(LangFamily::from("ecma")),
    tool: "node",
    node_bin: false,
    version: &["node", "--version"],
    command,
    env: &[],
    merged: false,
    read,
    completed_codes: &[0],
    // Test files are arguments `node --test` runs in place of the files it would find.
    select: files,
    exclude: keeps_every_test,
    list: None,
    // Node runs each file in its own process, its tests in the order they are declared, and has
    // no shuffle or seed: a run varies only in how many files it runs at once.
    vary: &Varying {
        ordering: RunOrdering::Seeded,
        args: concurrency,
        env: |_| Vec::new(),
        executed: Executed::Unknown,
    },
    outputs: &[],
    build: None,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_runner(Runner(&NODE_TEST)),
});

fn command(root: &Root) -> Vec<String> {
    vec![
        root.program.clone(),
        "--test".to_owned(),
        "--test-reporter=junit".to_owned(),
        format!("--test-reporter-destination={}", root.report.display()),
    ]
}

/// `--test-concurrency` from a run's parallelism.
fn concurrency(options: &RunOptions) -> Vec<String> {
    options
        .parallelism
        .map(|workers| format!("--test-concurrency={workers}"))
        .into_iter()
        .collect()
}

fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    junit::node(ran.report.as_deref()?, &ran.root.dir)
}
