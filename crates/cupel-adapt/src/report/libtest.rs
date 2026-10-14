// ABOUTME: Parses the text libtest prints through `cargo test` (stdout and stderr merged): each target's
// ABOUTME: `Running` line, its `test <name> ... <result>` lines, and the captured output of failures.
use std::collections::BTreeMap;

use cupel_core::{TestOutcome, TestResult, output_tail};

/// The tests in `text`, the merged output of `cargo test --no-fail-fast`, with IDs
/// `<binary>/<file>::<test path>` (ADR-075); libtest measures no durations on stable Rust.
pub fn parse(text: &str) -> Vec<TestResult> {
    let mut tests = Vec::new();
    let mut target: Option<Target> = None;
    let mut failures: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(found) = Target::of(line) {
            target = Some(found);
        } else if let (Some(target), Some((name, outcome))) = (&target, result(line)) {
            tests.push(target.test(name, outcome));
        } else if let (Some(target), Some(name)) = (&target, failure_header(line)) {
            let mut output = Vec::new();
            while let Some(next) = lines.peek() {
                if failure_header(next).is_some() || *next == "failures:" {
                    break;
                }
                output.push(lines.next().unwrap_or_default());
            }
            failures.insert((target.binary.clone(), name.to_owned()), output.join("\n"));
        }
    }
    for test in &mut tests {
        if let Some(output) = failures.get(&(binary_of(&test.id), test.name.clone())) {
            test.output = output_tail(output);
        }
    }
    tests
}

/// A test binary libtest is running: its `target/debug/deps` stem and the file it was built from.
struct Target {
    binary: String,
    file: Option<String>,
}

impl Target {
    /// `     Running unittests src/lib.rs (target/debug/deps/tally-a419…)`, `     Running
    /// tests/red.rs (target/debug/deps/red-168a…)`, or `   Doc-tests tally`.
    fn of(line: &str) -> Option<Self> {
        let line = line.trim_start();
        if let Some(krate) = line.strip_prefix("Doc-tests ") {
            return Some(Self {
                binary: format!("{}-doc", krate.trim()),
                file: None,
            });
        }
        let rest = line.strip_prefix("Running ")?;
        let (source, binary) = rest.rsplit_once(" (")?;
        let file = source.strip_prefix("unittests ").unwrap_or(source).trim();
        let stem = binary.trim_end_matches(')').rsplit('/').next()?;
        let binary = stem.rsplit_once('-').map_or(stem, |(name, _)| name);
        Some(Self {
            binary: binary.to_owned(),
            file: Some(file.to_owned()),
        })
    }

    fn test(&self, name: &str, outcome: TestOutcome) -> TestResult {
        // A doctest names its file itself: `src/lib.rs - total (line 3)`.
        let file = self
            .file
            .clone()
            .or_else(|| name.split_once(" - ").map(|(file, _)| file.to_owned()));
        TestResult {
            id: format!(
                "{}/{}::{name}",
                self.binary,
                self.file.as_deref().unwrap_or("doc")
            ),
            file,
            name: name.to_owned(),
            outcome,
            duration_ms: None,
            output: None,
        }
    }
}

/// The binary part of an ID made by [`Target::test`].
fn binary_of(id: &str) -> String {
    id.split_once('/')
        .map_or(id, |(binary, _)| binary)
        .to_owned()
}

/// `test tests::totals_a_list ... ok` → the name and outcome.
fn result(line: &str) -> Option<(&str, TestOutcome)> {
    let rest = line.strip_prefix("test ")?;
    let (name, result) = rest.rsplit_once(" ... ")?;
    let outcome = match result.split(',').next()?.trim() {
        "ok" => TestOutcome::Passed,
        "FAILED" => TestOutcome::Failed,
        "ignored" => TestOutcome::Skipped,
        _ => return None,
    };
    Some((name, outcome))
}

/// `---- tests::totals_a_list stdout ----` → the test's name.
fn failure_header(line: &str) -> Option<&str> {
    line.strip_prefix("---- ")?.strip_suffix(" stdout ----")
}
