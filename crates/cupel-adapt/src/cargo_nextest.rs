// ABOUTME: The `cargo nextest` runner producer: `cargo nextest run` with a tool configuration file whose `cupel`
// ABOUTME: profile writes a JUnit report under the target directory; it builds once and reruns that build (ADR-150 §8).
use cupel_core::{LangId, Langs, RunOptions, RunOrdering, TestResult};

use crate::report::{junit, listing};
use std::path::PathBuf;

use crate::runner::{
    Builder, Executed, Lister, Ran, Record, Root, Runner, RunnerSpec, Varying, tests_flag,
};

const ID: &str = "tool.cargo-nextest";

pub(crate) static CARGO_NEXTEST: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("rust")]),
    tool: "cargo-nextest",
    node_bin: false,
    version: &["cargo", "nextest", "--version"],
    command,
    // nextest hands its profile to the tests it runs as `NEXTEST_PROFILE`, which a nextest run
    // inside one of them reads; `run` names its own, and `list` takes the default.
    env: &[
        ("CARGO_TERM_COLOR", "never"),
        ("NEXTEST_PROFILE", "default"),
    ],
    merged: false,
    read,
    select: |selected| tests_flag(selected, "-E", filterset),
    exclude: |ids| Some(vec!["-E".to_owned(), format!("not ({})", filterset(ids))]),
    list: Some(Lister {
        command: list_command,
        parse: listing::nextest,
    }),
    completed_codes: &[0],
    // One process per test has no order or seed to vary; the run's parallelism is its test
    // threads and build jobs (ADR-148).
    vary: &Varying {
        ordering: RunOrdering::Fixed,
        args: vary,
        env: |_| Vec::new(),
        executed: Executed::Unknown,
    },
    outputs: &["target"],
    build: Some(&BUILD),
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_runner(Runner(&CARGO_NEXTEST)),
});

/// The `cupel` profile: a `JUnit` report that keeps the output of failures.
pub(crate) const PROFILE: &str = "[profile.cupel.junit]\npath = \"junit.xml\"\nstore-success-output = false\nstore-failure-output = true\n";

fn command(root: &Root) -> Vec<String> {
    let mut command = words(&["cargo", "nextest", "run", "--workspace"]);
    command.extend(profile(root));
    command
}

/// The run's profile: no early stop, and the `cupel` profile from the tool configuration file,
/// which is written here (in `.cupel/`; a failure to write it surfaces as nextest's error).
fn profile(root: &Root) -> Vec<String> {
    let _ = std::fs::write(&root.config, PROFILE);
    let mut words = words(&["--no-fail-fast", "--profile", "cupel", "--tool-config-file"]);
    words.push(format!("cupel:{}", root.config.display()));
    words
}

fn words(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// Building once (ADR-150 §8): nextest's record of the test binaries it built, and cargo's of
/// the workspace, which a run elsewhere reads in place of building.
static BUILD: Builder = Builder {
    records: build_records,
    command: reuse_command,
    vary: reuse_vary,
};

/// `cargo nextest list` builds every test binary of the workspace (with the build's jobs) and
/// prints what it built; `cargo metadata` prints the workspace.
fn build_records(_: &Root, options: &RunOptions) -> Vec<Record> {
    let mut list = words(&[
        "cargo",
        "nextest",
        "list",
        "--workspace",
        "--list-type",
        "binaries-only",
        "--message-format",
        "json",
    ]);
    if let Some(jobs) = options.parallelism {
        list.extend(["--build-jobs".to_owned(), jobs.to_string()]);
    }
    vec![
        ("binaries.json", list),
        (
            "cargo-metadata.json",
            words(&["cargo", "metadata", "--format-version", "1"]),
        ),
    ]
}

/// A run, in `root`, of the binaries `records` describe: no build, `root` as the workspace its
/// tests run in, and none of cargo's build options, which nextest refuses there.
fn reuse_command(root: &Root, records: &[PathBuf]) -> Vec<String> {
    let mut command = words(&["cargo", "nextest", "run"]);
    command.extend(profile(root));
    for (flag, record) in ["--binaries-metadata", "--cargo-metadata"]
        .into_iter()
        .zip(records)
    {
        command.extend([flag.to_owned(), record.display().to_string()]);
    }
    command.extend([
        "--workspace-remap".to_owned(),
        root.dir.display().to_string(),
    ]);
    command
}

/// A run from the build takes the run's parallelism as its test threads only.
fn reuse_vary(options: &RunOptions) -> Vec<String> {
    options
        .parallelism
        .map(|threads| vec!["--test-threads".to_owned(), threads.to_string()])
        .unwrap_or_default()
}

/// A run's parallelism as its test threads and its build jobs; nothing for the runner's
/// defaults.
fn vary(options: &RunOptions) -> Vec<String> {
    options
        .parallelism
        .map(|parallelism| {
            let count = parallelism.to_string();
            vec![
                "--test-threads".to_owned(),
                count.clone(),
                "--build-jobs".to_owned(),
                count,
            ]
        })
        .unwrap_or_default()
}

fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    let report = std::fs::read_to_string(
        ran.root
            .dir
            .join("target")
            .join("nextest")
            .join("cupel")
            .join("junit.xml"),
    )
    .ok()?;
    junit::nextest(&report)
}

/// The filterset that matches exactly the nextest tests `ids` (`<binary-id> <test path>`).
pub(crate) fn filterset(ids: &[String]) -> String {
    ids.iter()
        .map(|id| match id.split_once(' ') {
            Some((binary, test)) => format!("(binary_id(={binary}) & test(={test}))"),
            None => format!("test(={id})"),
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn list_command(_: &Root) -> Vec<String> {
    [
        "cargo",
        "nextest",
        "list",
        "--workspace",
        "--message-format",
        "json",
    ]
    .map(str::to_owned)
    .to_vec()
}
