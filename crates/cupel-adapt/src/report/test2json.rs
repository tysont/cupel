// ABOUTME: Parses `go test -json` (test2json) events: each test's final action and elapsed time, its output
// ABOUTME: for failures, and a package that failed with no failing test or a build that failed as errored.
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
    /// The package a `build-output` or `build-fail` event is about.
    #[serde(default)]
    import_path: Option<String>,
    #[serde(default)]
    test: Option<String>,
    #[serde(default)]
    elapsed: Option<f64>,
    #[serde(default)]
    output: Option<String>,
    /// The build whose failure failed a package, as its `build-output` events name it.
    #[serde(default)]
    failed_build: Option<String>,
}

/// What the events say about one test, one package, or one build.
#[derive(Default)]
struct Seen {
    outcome: Option<TestOutcome>,
    elapsed: Option<f64>,
    output: String,
    failed_build: Option<String>,
}

/// Every test, package, and build the events mention, with what they say about it.
#[derive(Default)]
struct Sightings {
    tests: BTreeMap<(String, String), Seen>,
    packages: BTreeMap<String, Seen>,
    builds: BTreeMap<String, Seen>,
}

/// The tests in `stdout`, the events of `go test -json`, with IDs `<package>/<Test>`. A package
/// that failed without a failing test is one `errored` test named after the package, its output
/// the output of the build that failed it, then its own, then `stderr` (where `go` before 1.24
/// reports build errors). A build that failed and failed no package (as when the toolchain lacks
/// `covdata`) is one `errored` test named after the package it built, with its output and
/// `stderr`. Lines that are not events are skipped.
pub fn parse(stdout: &str, stderr: &str) -> Vec<TestResult> {
    let Sightings {
        tests,
        packages,
        builds,
    } = events(stdout);
    let errored = |id: &str, output: String| TestResult {
        id: id.to_owned(),
        file: None,
        name: id.to_owned(),
        outcome: TestOutcome::Errored,
        duration_ms: None,
        output: output_tail(&format!("{output}\n{stderr}")),
    };
    let failing: Vec<&str> = tests
        .iter()
        .filter(|(_, seen)| seen.outcome == Some(TestOutcome::Failed))
        .map(|((package, _), _)| package.as_str())
        .collect();
    let broken = packages.iter().filter(|(package, seen)| {
        seen.outcome == Some(TestOutcome::Failed) && !failing.contains(&package.as_str())
    });
    let build_output = |build: &Option<String>| {
        let seen = build.as_ref().and_then(|build| builds.get(build));
        seen.map(|seen| seen.output.as_str()).unwrap_or_default()
    };
    let mut results: Vec<TestResult> = broken
        .map(|(package, seen)| {
            let output = format!("{}{}", build_output(&seen.failed_build), seen.output);
            errored(package, output)
        })
        .collect();
    results.extend(
        unclaimed(&builds, &packages).map(|(build, seen)| errored(build, seen.output.clone())),
    );
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

/// The builds in `builds` that failed and that no package in `packages` names as the build that
/// failed it, by the package they built (an import path without its ` [<test>]` variant).
fn unclaimed<'a>(
    builds: &'a BTreeMap<String, Seen>,
    packages: &'a BTreeMap<String, Seen>,
) -> impl Iterator<Item = (&'a str, &'a Seen)> {
    builds
        .iter()
        .filter(|(_, seen)| seen.outcome == Some(TestOutcome::Failed))
        .filter(|(build, _)| {
            !packages
                .values()
                .any(|seen| seen.failed_build.as_ref() == Some(*build))
        })
        .map(|(build, seen)| (build.split(" [").next().unwrap_or(build), seen))
}

/// What the events in `stdout` say about each test, each package, and each build.
fn events(stdout: &str) -> Sightings {
    let mut sightings = Sightings::default();
    for event in stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Event>(line).ok())
    {
        let seen = match (&event.import_path, &event.test) {
            (Some(build), _) => sightings.builds.entry(build.clone()).or_default(),
            (None, Some(test)) => sightings
                .tests
                .entry((event.package.clone(), test.clone()))
                .or_default(),
            (None, None) => sightings.packages.entry(event.package.clone()).or_default(),
        };
        seen.record(&event);
    }
    sightings
}

impl Seen {
    fn record(&mut self, event: &Event) {
        match event.action.as_str() {
            "pass" => self.outcome = Some(TestOutcome::Passed),
            "fail" | "build-fail" => self.outcome = Some(TestOutcome::Failed),
            "skip" => self.outcome = Some(TestOutcome::Skipped),
            "output" | "build-output" => self
                .output
                .push_str(event.output.as_deref().unwrap_or_default()),
            _ => {}
        }
        if event.elapsed.is_some() {
            self.elapsed = event.elapsed;
        }
        if event.failed_build.is_some() {
            self.failed_build.clone_from(&event.failed_build);
        }
    }
}

/// What each test printed, then what each package printed outside its tests, as
/// `(package, test, output)`, from `stdout`, the events of `go test -json`.
pub fn printed(stdout: &str) -> Vec<(String, Option<String>, String)> {
    let Sightings {
        tests, packages, ..
    } = events(stdout);
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
