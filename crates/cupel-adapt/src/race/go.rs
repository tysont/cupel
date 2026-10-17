// ABOUTME: `tool.go-race` (ADR-105 §2): a Go root's packages run with `go test -race -json`, each race report in a test's
// ABOUTME: output read into its two access stacks; a package that did not build is a failed run.
use cupel_core::{
    Availability, Captured, CostClass, Ctx, Process, ProducerId, RaceKind, TestOutcome, ToolId,
    output_tail,
};

use super::report::{Raw, RawFrame, RawStack, Read, access, place};
use super::{Invocation, Spec};
use crate::go_test::packages;
use crate::report::test2json;
use crate::runner::installed;
use crate::toolchain;

const ID: &str = "tool.go-race";

pub static SPEC: Spec = Spec {
    id: ID,
    lang: "go",
    cost: CostClass::Standard,
    program: "go",
    version: &["go", "version"],
    availability,
    command,
    read,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| super::register(registry, &SPEC),
});

fn availability(ctx: &Ctx) -> Availability {
    if installed("go", false, &ctx.root).is_some() {
        Availability::Ready
    } else {
        Availability::MissingTool {
            name: ToolId::from("go"),
            install: toolchain::install(&ProducerId::from(ID), "go", &ctx.root),
        }
    }
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "every detector builds its command through `Spec::command`; this one cannot fail"
)]
fn command(invocation: &Invocation<'_>) -> cupel_core::Result<Process> {
    Ok(Process::new("go", invocation.dir)
        .args(["test", "-race", "-count=1", "-json"])
        .args(packages(invocation.packages)))
}

/// The reports in each test's output (and in a package's, outside its tests); a run with no
/// events, or a package that failed without a failing test (it did not build), failed.
fn read(captured: &Captured) -> Read {
    let results = test2json::parse(&captured.stdout, &captured.stderr);
    let broken = results
        .iter()
        .find(|result| result.outcome == TestOutcome::Errored && result.id == result.name);
    let failed = match (results.is_empty(), broken) {
        (true, _) => {
            Some(output_tail(&captured.stderr).unwrap_or_else(|| "no test ran".to_owned()))
        }
        (false, Some(package)) => Some(package.output.clone().unwrap_or_default()),
        (false, None) => None,
    };
    let reports = test2json::printed(&captured.stdout)
        .into_iter()
        .flat_map(|(_, test, output)| reports(&output, test.as_deref()))
        .collect();
    Read { reports, failed }
}

/// Every report between `WARNING: DATA RACE` and the line of `=` that ends it in `output`, which
/// `test` printed.
fn reports(output: &str, test: Option<&str>) -> Vec<Raw> {
    let mut found = Vec::new();
    let mut current: Option<Report> = None;
    for line in output.lines() {
        if line == "WARNING: DATA RACE" {
            current = Some(Report::new(test));
        } else if line.starts_with("==================") {
            found.extend(current.take().map(|report| report.raw));
        } else if let Some(report) = current.as_mut() {
            report.read(line);
        }
    }
    found
}

/// A report being read.
struct Report {
    raw: Raw,
    /// Whether the lines read now are frames of an access (not a goroutine's creation).
    in_access: bool,
    /// The function of the frame whose place is the next line.
    function: Option<String>,
}

impl Report {
    fn new(test: Option<&str>) -> Self {
        Self {
            raw: Raw {
                kind: RaceKind::DataRace,
                test: test.map(str::to_owned),
                message: "data race".to_owned(),
                stacks: Vec::new(),
            },
            in_access: false,
            function: None,
        }
    }

    fn read(&mut self, line: &str) {
        if let Some(access) = access(line, " at ") {
            self.raw.stacks.push(RawStack {
                access: Some(access),
                frames: Vec::new(),
            });
            self.in_access = true;
        } else if !line.starts_with(' ') && !line.is_empty() {
            // `Goroutine 7 (running) created at:` and the like: not an access.
            self.in_access = false;
        } else if self.in_access && line.starts_with("      ") {
            self.place(line.trim());
        } else if self.in_access && line.starts_with("  ") {
            self.function = Some(called(line.trim()).to_owned());
        }
    }

    /// The frame at `text` (`/abs/file.go:10 +0x3c`), with the function the line before named;
    /// a relative place (`_testmain.go`) is generated code and is not kept.
    fn place(&mut self, text: &str) {
        let function = self.function.take();
        let Some((file, line)) = place(text.split(" +0x").next().unwrap_or(text)) else {
            return;
        };
        if !file.starts_with('/') {
            return;
        }
        if let Some(stack) = self.raw.stacks.last_mut() {
            stack.frames.push(RawFrame {
                function,
                file,
                line,
            });
        }
    }
}

/// A frame's function without the call's arguments (`pkg.F.func1(0x1, {…})` → `pkg.F.func1`);
/// parentheses inside the name (`testing.(*T).Run`) stay.
fn called(text: &str) -> &str {
    if !text.ends_with(')') {
        return text;
    }
    let mut depth = 0usize;
    for (at, c) in text.char_indices().rev() {
        match c {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth == 0 {
                    return &text[..at];
                }
            }
            _ => {}
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use cupel_core::RaceReport;
    use cupel_fixtures::Capture;

    use super::*;
    use crate::race::report::Paths;

    fn captured(stdout: &str, stderr: &str) -> Captured {
        Captured {
            exit_code: Some(1),
            stdout: stdout.to_owned(),
            stderr: stderr.to_owned(),
        }
    }

    #[test]
    fn the_race_in_a_test_is_its_write_and_read_with_their_frames_in_the_repository() {
        let capture = Capture::of("race/go-race", "race");
        let read = read(&captured(&capture.file("stdout.jsonl"), ""));
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
        let frame = |function: &str, file: &str, line: u32| serde_json::json!({"function": function, "file": file, "line": line});
        assert_eq!(
            serde_json::to_value(&reports).unwrap(),
            serde_json::json!([{
                "kind": "data-race",
                "test": "TestPublish",
                "message": "data race",
                "stacks": [
                    {"access": "write", "frames": [frame("example.com/tally.Publish.func1", "tally.go", 10)]},
                    {"access": "read", "frames": [
                        frame("example.com/tally.Publish", "tally.go", 13),
                        frame("example.com/tally.TestPublish", "tally_test.go", 6),
                    ]},
                ],
            }])
        );
    }

    #[test]
    fn no_events_or_a_package_that_did_not_build_is_a_failed_run() {
        let cgo = read(&captured(
            "",
            "go: -race requires cgo; enable cgo by setting CGO_ENABLED=1\n",
        ));
        assert_eq!(
            cgo.failed.as_deref(),
            Some("go: -race requires cgo; enable cgo by setting CGO_ENABLED=1")
        );
        let broken = Capture::of("go-test", "broken");
        let unbuilt = read(&captured(&broken.file("stdout.jsonl"), ""));
        assert!(unbuilt.failed.is_some());
        assert!(unbuilt.reports.is_empty());
    }

    #[test]
    fn a_call_loses_its_arguments_and_keeps_parentheses_in_its_name() {
        assert_eq!(
            called("example.com/tally.Publish.func1()"),
            "example.com/tally.Publish.func1"
        );
        assert_eq!(called("pkg.F.func1(0x14000, {0x0?, 0x0?})"), "pkg.F.func1");
        assert_eq!(
            called("testing.(*T).Run.gowrap1()"),
            "testing.(*T).Run.gowrap1"
        );
        assert_eq!(
            access("Read at 0x00c00019a228 by main goroutine:", " at ").as_deref(),
            Some("read")
        );
        assert_eq!(access("Goroutine 7 (running) created at:", " at "), None);
    }
}
