// ABOUTME: The Maven runner producer: `mvn test`, Surefire's JUnit reports (one per test class, in each module's
// ABOUTME: `target/surefire-reports`, emptied before the run) read by the JUnit parser; `-Dtest` narrows (ADR-108).
use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::{LangId, Langs, RunOptions, RunOrder, RunOrdering, TestResult};

use crate::java_tests::{class_selection, read_reports};
use crate::runner::{Executed, Ran, Root, Runner, RunnerSpec, Varying, keeps_every_test};

const ID: &str = "tool.maven";

static MAVEN: RunnerSpec = RunnerSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("java")]),
    tool: "mvn",
    node_bin: false,
    version: &["mvn", "--version"],
    command,
    env: &[],
    merged: false,
    read,
    select: |selected| class_selection(selected, '+').map(|tests| vec![format!("-Dtest={tests}")]),
    exclude: keeps_every_test,
    list: None,
    completed_codes: &[0],
    vary: &Varying {
        ordering: RunOrdering::Seeded,
        args: vary,
        env: |_| Vec::new(),
        // One report per class, read in name order: the reports do not say the order classes ran in.
        executed: Executed::Unknown,
    },
    outputs: &["target"],
    build: None,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_runner(Runner(&MAVEN)),
});

/// Where Surefire writes its reports in `root`: each module's `target/surefire-reports`, the
/// root's and those of the modules one level below it. Surefire's report directory has no
/// property a command line can set, so the runner reads where it writes.
fn reports(root: &Path) -> Vec<PathBuf> {
    let modules = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|dir| dir.join("pom.xml").is_file());
    let mut dirs: Vec<PathBuf> = std::iter::once(root.to_path_buf())
        .chain(modules)
        .map(|dir| dir.join("target").join("surefire-reports"))
        .collect();
    dirs.sort();
    dirs
}

/// `mvn test`, each report directory emptied first so the run reads only its own reports; a
/// narrowed run that names no test of some module does not fail it.
fn command(root: &Root) -> Vec<String> {
    for dir in reports(&root.dir) {
        // A failure to clear one surfaces as stale reports, which the next run's clear removes.
        let _ = fs::remove_dir_all(dir);
    }
    vec![
        root.program.clone(),
        "-B".to_owned(),
        "--no-transfer-progress".to_owned(),
        "test".to_owned(),
        "-Dsurefire.failIfNoSpecifiedTests=false".to_owned(),
    ]
}

/// Surefire's order of test classes (ADR-078): alphabetical, so every machine runs a suite the
/// same way whatever its file system lists first, or reverse alphabetical for a shuffled run.
/// Surefire's `random` order shuffles the classes as the file system listed them, so one seed is
/// a different permutation on each platform; the reverse order puts every class after the ones it
/// followed, on every machine alike, and the seed is left to the methods. Methods keep `JUnit`'s
/// order within a class, and a `pom.xml` that sets `runOrder` keeps its own.
fn vary(options: &RunOptions) -> Vec<String> {
    let order = match options.order {
        RunOrder::Shuffled => "reversealphabetical",
        _ => "alphabetical",
    };
    vec![format!("-Dsurefire.runOrder={order}")]
}

fn read(ran: &Ran<'_>) -> Option<Vec<TestResult>> {
    read_reports(ran, &reports(&ran.root.dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shuffled_run_reverses_the_alphabetical_class_order_whatever_the_seed() {
        let shuffled = RunOptions {
            order: RunOrder::Shuffled,
            seed: Some(7),
            ..RunOptions::default()
        };
        assert_eq!(vary(&shuffled), ["-Dsurefire.runOrder=reversealphabetical"]);
        let declared = RunOptions {
            order: RunOrder::Declared,
            seed: Some(7),
            ..RunOptions::default()
        };
        assert_eq!(vary(&declared), ["-Dsurefire.runOrder=alphabetical"]);
    }
}
