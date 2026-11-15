// ABOUTME: The `pytest` runner producer: a JUnit report (`xunit1`, which names each test's file) written to `.cupel/`,
// ABOUTME: no cache directory and no bytecode written into the tree; read by the JUnit parser.
use cupel_core::{LangId, Langs, TestResult};

use crate::report::{junit, listing};
use crate::runner::{Lister, PYTHON, Ran, Root, Runner, RunnerSpec, tests_or_files};

const ID: &str = "tool.pytest";

static PYTEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("python")]),
    tool: "pytest",
    node_bin: false,
    version: &["pytest", "--version"],
    command,
    env: &[("PYTHONDONTWRITEBYTECODE", "1")],
    merged: false,
    read,
    // Node IDs and test files are both arguments pytest runs.
    select: tests_or_files,
    exclude: |ids| {
        Some(
            ids.iter()
                .flat_map(|id| ["--deselect".to_owned(), id.clone()])
                .collect(),
        )
    },
    list: Some(Lister {
        command: collect,
        parse: listing::pytest,
    }),
    completed_codes: &[0, 5],
    vary: &PYTHON,
    outputs: &[],
    build: None,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_runner(Runner(&PYTEST)),
});

fn command(root: &Root) -> Vec<String> {
    let mut command: Vec<String> = [
        root.program.as_str(),
        "-p",
        "no:cacheprovider",
        "-o",
        "junit_family=xunit1",
        "-o",
        "junit_logging=all",
    ]
    .map(str::to_owned)
    .to_vec();
    command.push(format!("--junitxml={}", root.report.display()));
    command.extend(arguments(&root.arguments, root.narrowed));
    command
}

/// The configured arguments a pytest run takes (ADR-229): every one, or in a run narrowed to
/// tests Cupel names, the options with their values alone. pytest runs every path it is given,
/// so a configured path beside the named tests would run all of it. A word that does not start
/// with `-` is an option's value when it follows an option without `=`, and a path otherwise.
pub(crate) fn arguments(configured: &[String], narrowed: bool) -> Vec<String> {
    if !narrowed {
        return configured.to_vec();
    }
    let mut kept = Vec::new();
    let mut value_follows = false;
    for word in configured {
        let option = word.starts_with('-');
        if option || value_follows {
            kept.push(word.clone());
        }
        value_follows = option && !word.contains('=');
    }
    kept
}

fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    junit::pytest(ran.report.as_deref()?)
}

/// Collects the suite the configured arguments select without running it, one node ID per line.
fn collect(root: &Root) -> Vec<String> {
    let mut command: Vec<String> = [
        root.program.as_str(),
        "-p",
        "no:cacheprovider",
        "--collect-only",
        "-q",
    ]
    .map(str::to_owned)
    .to_vec();
    command.extend(root.arguments.iter().cloned());
    command
}

#[cfg(test)]
mod tests {
    use super::arguments;

    #[test]
    fn a_narrowed_run_keeps_the_configured_options_and_their_values_but_no_path() {
        let configured: Vec<String> = ["-m", "not network", "tests/unit", "-x", "--maxfail=2", "x"]
            .map(str::to_owned)
            .to_vec();
        assert_eq!(arguments(&configured, false), configured);
        assert_eq!(
            arguments(&configured, true),
            ["-m", "not network", "-x", "--maxfail=2"]
        );
    }
}
