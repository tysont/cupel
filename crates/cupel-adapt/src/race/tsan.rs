// ABOUTME: `tool.tsan` (ADR-105 §2): a Rust root's tests built with `-Zsanitizer=thread` against a rebuilt standard
// ABOUTME: library, each ThreadSanitizer warning read into its access stacks from compiler-rt's report format.
use std::path::Path;

use cupel_core::{
    Availability, Cancellation, Captured, CostClass, Ctx, Error, Install, Process, RaceKind, ToolId,
};

use super::cargo::{NIGHTLY, failure, progress};
use super::report::{Raw, RawFrame, RawStack, Read, access, after, place};
use super::{Invocation, Spec};

const ID: &str = "tool.tsan";

/// The instrumentation: every crate and the standard library built with `ThreadSanitizer`.
const SANITIZE: &str = "-Zsanitizer=thread";

pub static SPEC: Spec = Spec {
    id: ID,
    lang: "rust",
    cost: CostClass::Standard,
    program: "rustc",
    version: &["rustc", NIGHTLY, "--version"],
    availability,
    command,
    read,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| super::register(registry, &SPEC),
});

/// Ready when the nightly toolchain has the standard library's source, which `-Zbuild-std`
/// rebuilds with the sanitizer.
fn availability(ctx: &Ctx) -> Availability {
    let source = nightly(&ctx.root, &["--print", "sysroot"]).map(|sysroot| {
        Path::new(sysroot.trim())
            .join("lib/rustlib/src/rust/library")
            .is_dir()
    });
    if source.is_ok_and(|present| present) {
        Availability::Ready
    } else {
        Availability::MissingTool {
            name: ToolId::from("rust-src"),
            install: Some(Install::Command(
                "rustup toolchain install nightly --component rust-src".to_owned(),
            )),
        }
    }
}

/// What `rustc +nightly <args>` prints in `dir`.
fn nightly(dir: &Path, args: &[&str]) -> cupel_core::Result<String> {
    let ran = Process::new("rustc", dir)
        .args([NIGHTLY])
        .args(args.iter().copied())
        .run(&Cancellation::new())?;
    if ran.exit_code == Some(0) {
        Ok(ran.stdout)
    } else {
        Err(Error::Process {
            program: "rustc".to_owned(),
            message: ran.stderr,
        })
    }
}

/// `cargo +nightly test` of the root's test targets for the host, every binary even after one
/// fails, one test at a time, built with the sanitizer into the cache.
fn command(invocation: &Invocation<'_>) -> cupel_core::Result<Process> {
    let version = nightly(invocation.dir, &["-vV"])?;
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap_or_default()
        .to_owned();
    Ok(Process::new("cargo", invocation.dir)
        .args([NIGHTLY, "test", "-Zbuild-std", "--target", &host])
        .args(["--tests", "--no-fail-fast", "--", "--test-threads=1"])
        .env("RUSTFLAGS", SANITIZE)
        .env("RUSTDOCFLAGS", SANITIZE)
        .env("CARGO_TARGET_DIR", &invocation.cache.to_string_lossy())
        .merged())
}

/// Every warning between `WARNING: ThreadSanitizer:` and `SUMMARY: ThreadSanitizer:` in the
/// merged output.
fn read(captured: &Captured) -> Read {
    let mut reports = Vec::new();
    let mut test: Option<String> = None;
    let mut current: Option<Warning> = None;
    for line in captured.stdout.lines() {
        let line = match progress(line) {
            Some((name, rest)) => {
                test = Some(name.to_owned());
                rest
            }
            None => line,
        };
        if let Some(headline) = after(line, "WARNING: ThreadSanitizer: ") {
            current = Some(Warning::new(headline, test.clone()));
        } else if after(line, "SUMMARY: ThreadSanitizer:").is_some() {
            reports.extend(current.take().map(|warning| warning.raw));
        } else if let Some(warning) = current.as_mut() {
            warning.read(line.trim());
        }
    }
    let failed = failure(&captured.stdout, !reports.is_empty());
    Read { reports, failed }
}

/// A warning being read.
struct Warning {
    raw: Raw,
    /// Whether the lines read now are frames of an access (not a thread's creation or a
    /// location).
    in_access: bool,
}

impl Warning {
    fn new(headline: &str, test: Option<String>) -> Self {
        let kind = if headline.starts_with("data race") {
            RaceKind::DataRace
        } else {
            RaceKind::UndefinedBehavior
        };
        Self {
            raw: Raw {
                kind,
                test,
                message: headline.to_owned(),
                stacks: Vec::new(),
            },
            in_access: false,
        }
    }

    fn read(&mut self, line: &str) {
        if let Some(access) = access(line, " of size ") {
            self.raw.stacks.push(RawStack {
                access: Some(access),
                frames: Vec::new(),
            });
            self.in_access = true;
        } else if line.starts_with('#') {
            if let Some(frame) = self.in_access.then(|| frame(line)).flatten()
                && let Some(stack) = self.raw.stacks.last_mut()
            {
                stack.frames.push(frame);
            }
        } else if !line.is_empty() {
            // `Location is global …`, `Thread T2 (…) created by …`, `Mutex M0 …`: not an access.
            self.in_access = false;
        }
    }
}

/// A frame line, `#0 tally::publish::{{closure}} /repo/src/lib.rs:9:49 (race-1a2b+0x1234)`, on
/// Linux followed by `(BuildId: …)`; a frame without a place (`<null>`) is not kept.
fn frame(line: &str) -> Option<RawFrame> {
    let (_, mut rest) = line.split_once(' ')?;
    while let Some(at) = rest.rfind(" (")
        && rest.ends_with(')')
    {
        rest = &rest[..at];
    }
    let (function, location) = rest.rsplit_once(' ')?;
    let (file, line) = place(location)?;
    Some(RawFrame {
        function: Some(function.to_owned()),
        file,
        line,
    })
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;

    use super::*;
    use crate::race::cargo::merged;

    #[test]
    fn a_data_race_is_its_two_accesses_without_the_threads_creation() {
        let output = Capture::of("race/tsan", "race").file("output.txt");
        let read = read(&merged(&format!("running 1 test\n{output}")));
        let count = |access: &str| RawStack {
            access: Some(access.to_owned()),
            frames: vec![RawFrame {
                function: Some("count".to_owned()),
                file: "race.c".to_owned(),
                line: 7,
            }],
        };
        assert_eq!(
            read,
            Read {
                reports: vec![Raw {
                    kind: RaceKind::DataRace,
                    test: None,
                    message: "data race (pid=83925)".to_owned(),
                    stacks: vec![count("write"), count("write")],
                }],
                failed: None,
            }
        );
    }

    #[test]
    fn a_rust_frame_keeps_its_function_and_place_and_another_warning_is_undefined_behavior() {
        let output = "running 1 test\ntest publishes ... ==================\n\
            WARNING: ThreadSanitizer: heap-use-after-free (pid=7)\n  \
            Read of size 8 at 0x7b0400000010 by main thread:\n    \
            #0 tally::publish::h0123456789abcdef /repo/src/lib.rs:9:49 (race-1a2b+0x1234)\n    \
            #1 core::ops::function::FnOnce::call_once <null> (race-1a2b+0x99)\n\n  \
            Previous write of size 8 at 0x7b0400000010 by thread T1 (mutexes: write M0):\n    \
            #0 std::thread::spawn /rustc/abc/library/std/src/thread/mod.rs:680:5 (race-1a2b+0x55)\n\n\
            SUMMARY: ThreadSanitizer: heap-use-after-free /repo/src/lib.rs:9:49 in tally::publish\n";
        let read = read(&merged(output));
        let report = &read.reports[0];
        assert_eq!(
            (report.kind, report.test.as_deref(), report.message.as_str()),
            (
                RaceKind::UndefinedBehavior,
                Some("publishes"),
                "heap-use-after-free (pid=7)"
            )
        );
        assert_eq!(
            report.stacks[0],
            RawStack {
                access: Some("read".to_owned()),
                frames: vec![RawFrame {
                    function: Some("tally::publish::h0123456789abcdef".to_owned()),
                    file: "/repo/src/lib.rs".to_owned(),
                    line: 9,
                }],
            }
        );
        assert_eq!(report.stacks[1].access.as_deref(), Some("write"));
    }

    #[test]
    fn a_linux_frame_keeps_its_place_before_the_module_offset_and_the_build_id() {
        let line = "#0 tally::publish::h0123456789abcdef /repo/src/lib.rs:9:49 (race-1a2b+0xf1bf8) \
            (BuildId: 066ac18022ff88ecd8b4eee0a402950043686a4d)";
        assert_eq!(
            frame(line),
            Some(RawFrame {
                function: Some("tally::publish::h0123456789abcdef".to_owned()),
                file: "/repo/src/lib.rs".to_owned(),
                line: 9,
            })
        );
        assert_eq!(
            frame(
                "#0 pthread_create <null> (r+0x68d38) (BuildId: 066ac18022ff88ecd8b4eee0a402950043686a4d)"
            ),
            None
        );
    }
}
