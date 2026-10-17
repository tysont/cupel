// ABOUTME: Parses the Jest JSON report, which `jest --json` writes and `vitest --reporter=json` writes in the
// ABOUTME: same shape: per test file, each test's titles, status, duration, and failure messages.
use std::path::Path;

use cupel_core::{TestOutcome, TestResult, output_tail};
use serde::Deserialize;

use super::milliseconds;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    test_results: Vec<FileResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileResult {
    /// The test file's absolute path.
    name: String,
    status: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    assertion_results: Vec<Assertion>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Assertion {
    #[serde(default)]
    ancestor_titles: Vec<String>,
    title: String,
    full_name: String,
    status: String,
    duration: Option<f64>,
    #[serde(default)]
    failure_messages: Vec<String>,
}

/// The tests in `json`, a report of a run in `dir`, with files relative to `dir` and IDs
/// `<file> > <ancestor titles> > <title>` (ADR-075). A test file that failed without running a
/// test (it did not load) is one `errored` test named after the file. `None` when `json` is not
/// such a report.
pub fn parse(json: &str, dir: &Path) -> Option<Vec<TestResult>> {
    let report: Report = serde_json::from_str(json).ok()?;
    let mut tests = Vec::new();
    for file in report.test_results {
        let path = relative(&file.name, dir);
        if file.assertion_results.is_empty() && file.status == "failed" {
            tests.push(TestResult {
                id: path.clone(),
                file: Some(path.clone()),
                name: path,
                outcome: TestOutcome::Errored,
                duration_ms: None,
                output: output_tail(&file.message),
            });
            continue;
        }
        for assertion in file.assertion_results {
            let outcome = outcome(&assertion.status);
            let id = std::iter::once(path.as_str())
                .chain(assertion.ancestor_titles.iter().map(String::as_str))
                .chain(std::iter::once(assertion.title.as_str()))
                .collect::<Vec<_>>()
                .join(" > ");
            tests.push(TestResult {
                id,
                file: Some(path.clone()),
                name: assertion.full_name,
                outcome,
                duration_ms: assertion.duration.and_then(milliseconds),
                output: outcome
                    .is_failure()
                    .then(|| output_tail(&without_frames(&assertion.failure_messages)))
                    .flatten(),
            });
        }
    }
    Some(tests)
}

/// Failure `messages` joined, without their stack frames (`    at f (file:1:2)`): a message's own
/// lines, such as a property's counterexample, are what its output tail must keep.
fn without_frames(messages: &[String]) -> String {
    messages
        .iter()
        .flat_map(|message| message.lines())
        .filter(|line| !line.trim_start().starts_with("at "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn outcome(status: &str) -> TestOutcome {
    match status {
        "passed" => TestOutcome::Passed,
        "failed" => TestOutcome::Failed,
        _ => TestOutcome::Skipped,
    }
}

/// `name` relative to `dir`, `/`-separated; `name` itself when it is not below `dir`.
fn relative(name: &str, dir: &Path) -> String {
    Path::new(name).strip_prefix(dir).map_or_else(
        |_| name.to_owned(),
        |path| {
            path.components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        },
    )
}
