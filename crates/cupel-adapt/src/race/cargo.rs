// ABOUTME: What the Rust race detectors share (ADR-105 §2): the nightly toolchain they run through `cargo +nightly`,
// ABOUTME: the test each libtest progress line names, and whether a cargo test run completed.
use std::path::Path;

use cupel_core::{Availability, Cancellation, Install, Process, ToolId, output_tail};

/// The toolchain both detectors need, as rustup names it.
pub const NIGHTLY: &str = "+nightly";

/// Ready when `probe` (a nightly command) succeeds in `root`; else `name` is missing and
/// `install` installs it.
pub fn available(root: &Path, probe: &[&str], name: &str, install: &str) -> Availability {
    let ran = Process::new(probe[0], root)
        .args(probe[1..].iter().copied())
        .run(&Cancellation::new());
    if ran.is_ok_and(|ran| ran.exit_code == Some(0)) {
        Availability::Ready
    } else {
        Availability::MissingTool {
            name: ToolId::from(name),
            install: Some(Install::Command(install.to_owned())),
        }
    }
}

/// The test libtest says it is running on `line` (`test publishes ... `), and the rest of the
/// line, where a detector's first line of output can follow on the same line.
pub fn progress(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("test ")?;
    let (name, after) = rest.split_once(" ... ")?;
    (!name.contains(' ')).then_some((name, after))
}

/// Why a `cargo test` run under a detector did not complete, from its merged `output`, when it
/// did not: a build error, a test binary killed by a signal with no report to show for it
/// (`reported` is whether the detector reported anything), or no test binary run at all.
pub fn failure(output: &str, reported: bool) -> Option<String> {
    let line = |prefix: &str| {
        output
            .lines()
            .find(|line| line.trim_start().starts_with(prefix))
            .map(|line| line.trim().to_owned())
    };
    if let Some(error) = line("error: could not compile").or_else(|| line("error[E")) {
        return Some(error);
    }
    if !reported && let Some(signal) = output.lines().find(|line| line.contains("(signal: ")) {
        let signal = signal.rsplit("(signal: ").next().unwrap_or_default();
        return Some(format!(
            "a test binary was killed by signal {}",
            signal.trim_end_matches(')')
        ));
    }
    if !output.lines().any(|line| line.starts_with("running ")) {
        return Some(output_tail(output).unwrap_or_else(|| "no test ran".to_owned()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_progress_line_names_its_test_and_keeps_what_follows() {
        assert_eq!(
            progress("test publishes ... error: Undefined Behavior: x"),
            Some(("publishes", "error: Undefined Behavior: x"))
        );
        assert_eq!(progress("test result: ok. 1 passed"), None);
        assert_eq!(progress("running 1 test"), None);
    }

    #[test]
    fn a_build_error_a_lone_signal_or_no_test_run_is_a_failure() {
        assert_eq!(
            failure("error[E0425]: cannot find value `x`\n", false).as_deref(),
            Some("error[E0425]: cannot find value `x`")
        );
        let crashed = "running 1 test\n  process didn't exit successfully: `race --test-threads=1` \
                       (signal: 11, SIGSEGV: invalid memory reference)\n";
        assert_eq!(
            failure(crashed, false).as_deref(),
            Some("a test binary was killed by signal 11, SIGSEGV: invalid memory reference")
        );
        assert_eq!(failure(crashed, true), None);
        assert_eq!(
            failure("error: no such command: `miri`\n", false).as_deref(),
            Some("error: no such command: `miri`")
        );
        assert_eq!(failure("running 2 tests\ntest result: ok.\n", false), None);
    }
}
