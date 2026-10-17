// ABOUTME: The `clojure.test` runner producer: the Clojure CLI with the `:test` alias runs Cupel's runner script,
// ABOUTME: which writes clojure.test's JUnit XML to `.cupel/`; test IDs are vars, read by the JUnit parser (ADR-109).
use std::path::{Path, PathBuf};

use cupel_core::{LangId, Langs, RunOptions, RunOrder, RunOrdering, TestResult};

use crate::report::junit;
use crate::runner::{Executed, Lister, Ran, Root, Runner, RunnerSpec, Varying, tests_or_files};

const ID: &str = "tool.clojure-test";

/// The runner script the CLI runs: loads the test namespaces, runs a selection, writes `JUnit` XML.
const SCRIPT: &str = include_str!("clojure_test.clj");

static CLOJURE_TEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("clojure")]),
    tool: "clojure",
    node_bin: false,
    version: &["clojure", "--version"],
    command,
    env: &[],
    merged: false,
    read,
    // Test IDs (vars) and test files are both arguments the script runs, in the order given.
    select: tests_or_files,
    list: Some(Lister {
        command: list_command,
        parse: |stdout| stdout.lines().map(str::to_owned).collect(),
    }),
    completed_codes: &[0],
    vary: &Varying {
        ordering: RunOrdering::Explicit,
        args: vary,
        env: |_| Vec::new(),
        executed: Executed::Report,
    },
    outputs: &["target", ".cpcache"],
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_runner(Runner(&CLOJURE_TEST)),
});

/// The runner script, written beside the runner's configuration file.
fn script(root: &Root) -> PathBuf {
    let script = root.config.with_file_name("cupel_test_runner.clj");
    // A failure to write it surfaces as the CLI's error that the file does not exist.
    let _ = std::fs::write(&script, SCRIPT);
    script
}

/// `clojure -M:test <script> <report>`: the `:test` alias puts the test paths on the classpath
/// (the CLI only warns when a project does not declare it). Narrowing names tests and files after.
fn command(root: &Root) -> Vec<String> {
    vec![
        root.program.clone(),
        "-M:test".to_owned(),
        script(root).display().to_string(),
        root.report.display().to_string(),
    ]
}

fn list_command(root: &Root) -> Vec<String> {
    let mut command = command(root);
    command.push("--list".to_owned());
    command
}

/// A shuffled run's seed, which the script shuffles its tests with (ADR-078).
fn vary(options: &RunOptions) -> Vec<String> {
    match (options.order, options.seed) {
        (RunOrder::Shuffled, Some(seed)) => vec!["--seed".to_owned(), seed.to_string()],
        _ => Vec::new(),
    }
}

/// The report's tests, each with the file of its namespace below the root's `test` or `src`.
fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    let mut tests = junit::clojure(ran.report.as_deref()?)?;
    for test in &mut tests {
        let namespace = test.id.split('/').next().unwrap_or_default();
        test.file = source_of(&ran.root.dir, namespace);
    }
    Some(tests)
}

/// The file defining `namespace` below `root`'s `test` or `src` directory, relative to `root`.
fn source_of(root: &Path, namespace: &str) -> Option<String> {
    let path = namespace.replace('.', "/").replace('-', "_");
    ["test", "src"]
        .into_iter()
        .flat_map(|dir| [".clj", ".cljc"].map(|extension| format!("{dir}/{path}{extension}")))
        .find(|candidate| root.join(candidate).is_file())
}
