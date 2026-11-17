// ABOUTME: The Leiningen runner producer (ADR-239): `lein with-profile +test run` runs Cupel's clojure.test runner script
// ABOUTME: on the project's test classpath, over the namespaces below `test/`; its JUnit XML is read as the Clojure CLI's is.
use cupel_core::{LangId, Langs, RunOrdering};

use crate::clojure_test::{read, script, vary};
use crate::runner::{
    Executed, Lister, Root, Runner, RunnerSpec, Varying, keeps_every_test, tests_or_files,
};

const ID: &str = "tool.lein-test";

/// The directory `lein test` runs by default, whose every namespace the script runs.
const TEST_DIR: &str = "test";

static LEIN_TEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("clojure")]),
    tool: "lein",
    node_bin: false,
    version: &["lein", "version"],
    command,
    env: &[],
    merged: false,
    read,
    // Test IDs (vars) and test files are both arguments the script runs, in the order given.
    select: tests_or_files,
    exclude: keeps_every_test,
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
    outputs: &["target"],
    build: None,
    prepare: None,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_runner(Runner(&LEIN_TEST)),
});

/// `lein with-profile +test run -m clojure.main <script> <report> --test-dir test`: the `:test`
/// profile's dependencies and paths join the default profiles', as under `lein test`, and
/// Leiningen runs Cupel's script with them. Narrowing names tests and files after.
fn command(root: &Root) -> Vec<String> {
    [
        root.program.as_str(),
        "with-profile",
        "+test",
        "run",
        "-m",
        "clojure.main",
    ]
    .map(str::to_owned)
    .into_iter()
    .chain([
        script(root).display().to_string(),
        root.report.display().to_string(),
        "--test-dir".to_owned(),
        TEST_DIR.to_owned(),
    ])
    .collect()
}

fn list_command(root: &Root) -> Vec<String> {
    let mut command = command(root);
    command.push("--list".to_owned());
    command
}
