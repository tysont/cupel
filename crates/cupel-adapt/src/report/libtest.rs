// ABOUTME: Parses the text libtest prints through `cargo test` (stdout and stderr merged): each target's
// ABOUTME: `Running` line, its `test <name> ... <result>` lines, and the captured output of failures.
use std::collections::BTreeMap;

use cupel_core::{TestOutcome, TestResult, output_tail};

/// The tests in `text`, the merged output of `cargo test --no-fail-fast`, with IDs
/// `<binary>/<file>::<test path>` (ADR-075); libtest measures no durations on stable Rust.
pub fn parse(text: &str) -> Vec<TestResult> {
    sections(text)
        .into_iter()
        .flat_map(|(target, lines)| target.tests(&lines))
        .collect()
}

/// `text` cut at each target's `Running` (or `Doc-tests`) line: each target with its lines.
fn sections(text: &str) -> Vec<(Target, Vec<&str>)> {
    let mut sections: Vec<(Target, Vec<&str>)> = Vec::new();
    for line in text.lines() {
        match Target::of(line) {
            Some(target) => sections.push((target, Vec::new())),
            None => {
                if let Some((_, lines)) = sections.last_mut() {
                    lines.push(line);
                }
            }
        }
    }
    sections
}

/// The captured output of each failed test in one target's `lines`, by test name: the lines
/// under its `---- <name> stdout ----` header, up to the next header or `failures:` line.
fn failure_outputs<'a>(lines: &[&'a str]) -> BTreeMap<&'a str, String> {
    let mut outputs = BTreeMap::new();
    let mut current: Option<(&str, Vec<&str>)> = None;
    for line in lines {
        let header = failure_header(line);
        if header.is_some() || *line == "failures:" {
            if let Some((name, body)) = current.take() {
                outputs.insert(name, body.join("\n"));
            }
            current = header.map(|name| (name, Vec::new()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push(line);
        }
    }
    if let Some((name, body)) = current {
        outputs.insert(name, body.join("\n"));
    }
    outputs
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

    /// The tests this target's `lines` report, each failure with its captured output.
    fn tests(&self, lines: &[&str]) -> Vec<TestResult> {
        let outputs = failure_outputs(lines);
        lines
            .iter()
            .filter_map(|line| result(line))
            .map(|(name, outcome)| TestResult {
                output: outputs.get(name).and_then(|output| output_tail(output)),
                ..self.test(name, outcome)
            })
            .collect()
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
