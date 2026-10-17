// ABOUTME: `tool.miri` (ADR-105 §2): a Rust root's tests run under Miri, each `Undefined Behavior` error read into a
// ABOUTME: report: a data race's two accesses, or where other undefined behavior happened and its backtrace.
use std::env;

use cupel_core::{Availability, Captured, CostClass, Ctx, Process, RaceKind};

use super::cargo::{NIGHTLY, available, failure, progress};
use super::report::{Raw, RawFrame, RawStack, Read, after, place};
use super::{Invocation, Spec};

const ID: &str = "tool.miri";

pub static SPEC: Spec = Spec {
    id: ID,
    lang: "rust",
    cost: CostClass::Deep,
    program: "cargo-miri",
    version: &["cargo", NIGHTLY, "miri", "--version"],
    availability,
    command,
    read,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| super::register(registry, &SPEC),
});

fn availability(ctx: &Ctx) -> Availability {
    available(
        &ctx.root,
        &["cargo", NIGHTLY, "miri", "--version"],
        "cargo-miri",
        "rustup toolchain install nightly --component miri",
    )
}

/// `cargo +nightly miri test`, every binary even after one fails, one test at a time (so each
/// error follows the name of its test), with full backtraces (which name the functions).
#[allow(
    clippy::unnecessary_wraps,
    reason = "every detector builds its command through `Spec::command`; this one cannot fail"
)]
fn command(invocation: &Invocation<'_>) -> cupel_core::Result<Process> {
    let flags = match env::var("MIRIFLAGS") {
        Ok(own) if !own.trim().is_empty() => format!("-Zmiri-backtrace=full {own}"),
        _ => "-Zmiri-backtrace=full".to_owned(),
    };
    Ok(Process::new("cargo", invocation.dir)
        .args([
            NIGHTLY,
            "miri",
            "test",
            "--no-fail-fast",
            "--",
            "--test-threads=1",
        ])
        .env("MIRIFLAGS", &flags)
        .env("CARGO_TARGET_DIR", &invocation.cache.to_string_lossy())
        .merged())
}

/// Every `Undefined Behavior` error in the merged output; an operation Miri does not support
/// (a foreign function, most file and process calls) ends that test binary and fails the run.
fn read(captured: &Captured) -> Read {
    let mut reading = Reading::default();
    for line in captured.stdout.lines() {
        reading.line(line);
    }
    reading.finish();
    let unsupported = captured
        .stdout
        .lines()
        .map(|line| progress(line).map_or(line, |(_, rest)| rest))
        .find_map(|line| after(line, "error: unsupported operation: "))
        .map(|what| format!("Miri does not support {what}"));
    let failed = unsupported.or_else(|| failure(&captured.stdout, !reading.reports.is_empty()));
    Read {
        reports: reading.reports,
        failed,
    }
}

/// Where in an error the lines being read are.
#[derive(Default, PartialEq, Eq)]
enum Part {
    #[default]
    Outside,
    /// After the message: the next `-->` is where it happened.
    Message,
    /// After `and (1) occurred earlier here`: the next `-->` is the earlier access.
    Earlier,
    /// In `stack backtrace:`.
    Backtrace,
    /// Elsewhere in the error (help, notes, source lines).
    Detail,
}

#[derive(Default)]
struct Reading {
    reports: Vec<Raw>,
    test: Option<String>,
    part: Part,
    error: Option<Error>,
}

/// An error being read.
struct Error {
    message: String,
    test: Option<String>,
    at: Option<RawFrame>,
    earlier: Option<RawFrame>,
    backtrace: Vec<RawFrame>,
    /// The function of the backtrace frame whose place is the next line.
    function: Option<String>,
}

impl Reading {
    fn line(&mut self, line: &str) {
        let line = match progress(line) {
            Some((test, rest)) => {
                self.test = Some(test.to_owned());
                rest
            }
            None => line,
        };
        if let Some(message) = after(line, "error: Undefined Behavior: ") {
            self.finish();
            self.error = Some(Error {
                message: message.to_owned(),
                test: self.test.clone(),
                at: None,
                earlier: None,
                backtrace: Vec::new(),
                function: None,
            });
            self.part = Part::Message;
        } else if after(line, "error: aborting due to").is_some() {
            self.finish();
        } else if let Some(error) = self.error.as_mut() {
            self.part = error.read(line, std::mem::take(&mut self.part));
        }
    }

    fn finish(&mut self) {
        self.reports.extend(self.error.take().map(Error::raw));
        self.part = Part::Outside;
    }
}

impl Error {
    /// Reads `line` in `part` of the error; the part the next line is in.
    fn read(&mut self, line: &str, part: Part) -> Part {
        if let Some(text) = after(line, "--> ") {
            let frame = place(text).map(|(file, line)| RawFrame {
                function: None,
                file,
                line,
            });
            match part {
                Part::Message => self.at = frame,
                Part::Earlier => self.earlier = frame,
                _ => {}
            }
            return Part::Detail;
        }
        if line
            .trim_start()
            .starts_with("help: and (1) occurred earlier here")
        {
            return Part::Earlier;
        }
        if line.contains("= note: stack backtrace:") {
            return Part::Backtrace;
        }
        if part != Part::Backtrace {
            return part;
        }
        self.backtrace_line(line.trim());
        Part::Backtrace
    }

    /// A backtrace line: `0: tally::publish` names a function, `at src/lib.rs:9:49: 9:63` places it.
    fn backtrace_line(&mut self, line: &str) {
        if let Some(text) = line.strip_prefix("at ") {
            if let Some((file, line)) = place(text) {
                self.backtrace.push(RawFrame {
                    function: self.function.take(),
                    file,
                    line,
                });
            }
        } else if let Some((index, function)) = line.split_once(": ")
            && index.chars().all(|c| c.is_ascii_digit())
        {
            self.function = Some(function.to_owned());
        }
    }

    /// The report: a data race's two accesses (the one that just happened with the backtrace,
    /// the earlier one at its place), or one stack where other undefined behavior happened.
    fn raw(self) -> Raw {
        let here: Vec<RawFrame> = if self.backtrace.is_empty() {
            self.at.into_iter().collect()
        } else {
            self.backtrace
        };
        if !self.message.starts_with("Data race detected") {
            return Raw {
                kind: RaceKind::UndefinedBehavior,
                test: self.test,
                message: self.message,
                stacks: vec![RawStack {
                    access: None,
                    frames: here,
                }],
            };
        }
        let stacks = vec![
            RawStack {
                access: operation(&self.message, "(2) "),
                frames: here,
            },
            RawStack {
                access: operation(&self.message, "(1) "),
                frames: self.earlier.into_iter().collect(),
            },
        ];
        Raw {
            kind: RaceKind::DataRace,
            test: self.test,
            message: self.message,
            stacks,
        }
    }
}

/// The operation a race message names after `marker` (`(1) non-atomic read on thread …`).
fn operation(message: &str, marker: &str) -> Option<String> {
    let (_, rest) = message.split_once(marker)?;
    let (operation, _) = rest.split_once(" on thread")?;
    Some(operation.to_owned())
}

#[cfg(test)]
mod tests {
    use cupel_core::{RaceFrame, RaceReport, RaceStack};
    use cupel_fixtures::Capture;

    use super::*;
    use crate::race::cargo::merged;
    use crate::race::report::Paths;

    #[test]
    fn a_race_and_an_out_of_bounds_read_are_two_reports_with_their_tests_and_places() {
        let capture = Capture::of("race/miri", "race-and-ub");
        let read = read(&merged(&capture.file("output.txt")));
        assert_eq!(read.failed, None);
        let paths = Paths {
            root: &capture.ran_in,
            dir: &capture.ran_in,
        };
        let reports: Vec<RaceReport> = read
            .reports
            .into_iter()
            .map(|raw| raw.normalized(&paths))
            .collect();
        let at = |function: Option<&str>, file: &str, line| RaceFrame {
            function: function.map(str::to_owned),
            file: file.to_owned(),
            line,
        };
        assert_eq!(
            reports,
            [
                RaceReport {
                    kind: RaceKind::DataRace,
                    test: Some("publishes".to_owned()),
                    message: "Data race detected between (1) non-atomic read on thread \
                              `publishes` and (2) non-atomic write on thread `unnamed-2` at alloc"
                        .to_owned(),
                    stacks: vec![
                        RaceStack {
                            access: Some("non-atomic write".to_owned()),
                            frames: vec![at(Some("tally::publish::{closure#0}"), "src/lib.rs", 9)],
                        },
                        RaceStack {
                            access: Some("non-atomic read".to_owned()),
                            frames: vec![at(None, "src/lib.rs", 10)],
                        },
                    ],
                },
                RaceReport {
                    kind: RaceKind::UndefinedBehavior,
                    test: Some("reads_the_last_value".to_owned()),
                    message: "memory access failed: attempting to access 1 byte, but got \
                              alloc+0x4 which is at or beyond the end of the allocation of size 4 \
                              bytes"
                        .to_owned(),
                    stacks: vec![RaceStack {
                        access: None,
                        frames: vec![
                            at(Some("tally::last"), "src/lib.rs", 18),
                            at(Some("reads_the_last_value"), "tests/ub.rs", 3),
                            at(Some("reads_the_last_value::{closure#0}"), "tests/ub.rs", 2),
                        ],
                    }],
                },
            ]
        );
    }

    #[test]
    fn an_unsupported_operation_fails_the_run_and_keeps_the_reports() {
        let output = "running 1 test\ntest reads_a_file ... error: unsupported operation: \
                      can't call foreign function `open` on OS `macos`\n";
        let read = read(&merged(output));
        assert_eq!(
            read.failed.as_deref(),
            Some("Miri does not support can't call foreign function `open` on OS `macos`")
        );
        assert!(read.reports.is_empty());
    }

    #[test]
    fn without_a_backtrace_undefined_behavior_is_placed_at_its_span() {
        let output = "running 1 test\ntest t ... error: Undefined Behavior: using uninitialized data\n  \
                      --> src/lib.rs:4:5\n   |\nerror: aborting due to 1 previous error\n";
        let read = read(&merged(output));
        let at = RawFrame {
            function: None,
            file: "src/lib.rs".to_owned(),
            line: 4,
        };
        assert_eq!(read.reports[0].stacks[0].frames, [at]);
        assert_eq!(read.reports[0].test.as_deref(), Some("t"));
    }
}
