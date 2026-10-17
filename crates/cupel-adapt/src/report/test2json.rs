// ABOUTME: Parses `go test -json` (test2json) events: each test's final action and elapsed time, its output
// ABOUTME: for failures, and a package that failed with no failing test (it did not build) as errored.
use std::collections::BTreeMap;

use cupel_core::{TestOutcome, TestResult, output_tail};
use serde::Deserialize;

use super::milliseconds;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Event {
    action: String,
    #[serde(default)]
    package: String,
    #[serde(default)]
    test: Option<String>,
    #[serde(default)]
    elapsed: Option<f64>,
    #[serde(default)]
    output: Option<String>,
}

/// What the events say about one test or one package.
#[derive(Default)]
struct Seen {
    outcome: Option<TestOutcome>,
    elapsed: Option<f64>,
    output: String,
}

/// Every test and package the events mention, with what they say about it.
type Sightings = (BTreeMap<(String, String), Seen>, BTreeMap<String, Seen>);

/// The tests in `stdout`, the events of `go test -json`, with IDs `<package>/<Test>`. A package
/// that failed without a failing test is one `errored` test named after the package, its output
/// followed by `stderr` (where `go` reports build errors). Lines that are not events are skipped.
pub fn parse(stdout: &str, stderr: &str) -> Vec<TestResult> {
    let (tests, packages) = events(stdout);
    let failing: Vec<&str> = tests
        .iter()
        .filter(|(_, seen)| seen.outcome == Some(TestOutcome::Failed))
        .map(|((package, _), _)| package.as_str())
        .collect();
    let broken = packages.iter().filter(|(package, seen)| {
        seen.outcome == Some(TestOutcome::Failed) && !failing.contains(&package.as_str())
    });
    let mut results: Vec<TestResult> = broken
        .map(|(package, seen)| TestResult {
            id: package.clone(),
            file: None,
            name: package.clone(),
            outcome: TestOutcome::Errored,
            duration_ms: None,
            output: output_tail(&format!("{}\n{stderr}", seen.output)),
        })
        .collect();
    results.extend(tests.into_iter().map(|((package, test), seen)| {
        // A test that started and never finished (the binary crashed) did not pass.
        let outcome = seen.outcome.unwrap_or(TestOutcome::Errored);
        TestResult {
            id: format!("{package}/{test}"),
            file: None,
            name: test,
            outcome,
            duration_ms: seen
                .elapsed
                .and_then(|seconds| milliseconds(seconds * 1000.0)),
            output: outcome
                .is_failure()
                .then(|| output_tail(&seen.output))
                .flatten(),
        }
    }));
    results
}

/// What the events in `stdout` say about each test and each package.
fn events(stdout: &str) -> Sightings {
    let mut tests: BTreeMap<(String, String), Seen> = BTreeMap::new();
    let mut packages: BTreeMap<String, Seen> = BTreeMap::new();
    for event in stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Event>(line).ok())
    {
        let seen = match &event.test {
            Some(test) => tests
                .entry((event.package.clone(), test.clone()))
                .or_default(),
            None => packages.entry(event.package.clone()).or_default(),
        };
        seen.record(&event);
    }
    (tests, packages)
}

impl Seen {
    fn record(&mut self, event: &Event) {
        match event.action.as_str() {
            "pass" => self.outcome = Some(TestOutcome::Passed),
            "fail" => self.outcome = Some(TestOutcome::Failed),
            "skip" => self.outcome = Some(TestOutcome::Skipped),
            "output" => self
                .output
                .push_str(event.output.as_deref().unwrap_or_default()),
            _ => {}
        }
        if event.elapsed.is_some() {
            self.elapsed = event.elapsed;
        }
    }
}

/// What each test printed, then what each package printed outside its tests, as
/// `(package, test, output)`, from `stdout`, the events of `go test -json`.
pub fn printed(stdout: &str) -> Vec<(String, Option<String>, String)> {
    let (tests, packages) = events(stdout);
    tests
        .into_iter()
        .map(|((package, test), seen)| (package, Some(test), seen.output))
        .chain(
            packages
                .into_iter()
                .map(|(package, seen)| (package, None, seen.output)),
        )
        .collect()
}

/// Each test's ID (`<package>/<Test>`) in the order `stdout`, the events of `go test -json`,
/// started them: its `run` events, in the order they appear.
pub fn order(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Event>(line).ok())
        .filter(|event| event.action == "run")
        .filter_map(|event| Some(format!("{}/{}", event.package, event.test?)))
        .collect()
}
