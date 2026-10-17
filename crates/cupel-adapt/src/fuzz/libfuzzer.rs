// ABOUTME: What every libFuzzer-based fuzzer (cargo-fuzz, atheris, Jazzer.js) shares: the flags that hand it the budget,
// ABOUTME: seed, limits, and where saved inputs go, and the reader of how a run ended from its output.
use std::path::{Path, PathBuf};

use cupel_core::{Cancellation, Captured, FuzzStatus, Location, Process};

use super::{Budget, Ended, INPUT_SECONDS, TARGET_SECONDS, last_count};

/// libFuzzer's flags for one target: the budget's executions and seed, the per-input limit, the
/// wall-clock cap, saved inputs under `area/inputs/`, and the final statistics.
pub fn flags(budget: &Budget, area: &Path) -> Vec<String> {
    vec![
        format!("-runs={}", budget.cases),
        format!("-seed={}", budget.libfuzzer_seed()),
        format!("-timeout={INPUT_SECONDS}"),
        format!("-max_total_time={TARGET_SECONDS}"),
        format!("-artifact_prefix={}/", area.join("inputs").display()),
        "-print_final_stats=1".to_owned(),
    ]
}

/// How a libFuzzer-based fuzzer is started on one target.
pub struct Invocation<'a> {
    pub program: &'a str,
    /// Where it runs.
    pub dir: &'a Path,
    /// Its arguments before the corpora, such as the target's module.
    pub before: Vec<String>,
    /// Variables added to its environment.
    pub env: Vec<(&'static str, &'static str)>,
    /// Whether libFuzzer's flags follow a `--` (Jazzer.js), rather than the corpora directly.
    pub separated: bool,
}

/// Runs `invocation` on the target's corpora (see [`corpora`]) with the budget's [`flags`],
/// and reads how it ended.
pub fn run(
    invocation: Invocation<'_>,
    area: &Path,
    committed: Option<PathBuf>,
    budget: &Budget,
    cancel: &Cancellation,
) -> cupel_core::Result<Ended> {
    let mut process = Process::new(invocation.program, invocation.dir);
    for (key, value) in invocation.env {
        process = process.env(key, value);
    }
    let ran = process
        .args(invocation.before)
        .args(corpora(area, committed))
        .args(invocation.separated.then(|| "--".to_owned()))
        .args(flags(budget, area))
        .merged()
        .run(cancel)?;
    Ok(ended(&ran))
}

/// The corpus directories of one target: its fresh one under `area` first (libFuzzer adds new
/// inputs to the first), then `committed` when it exists, which libFuzzer only reads.
pub fn corpora(area: &Path, committed: Option<PathBuf>) -> Vec<String> {
    std::iter::once(area.join("corpus"))
        .chain(committed.filter(|dir| dir.is_dir()))
        .map(|dir| dir.to_string_lossy().into_owned())
        .collect()
}

/// How a libFuzzer run ended, from its merged output: the input it saved names a crash
/// (`crash-`, `oom-`, `leak-`, and every other prefix) or a hang (`timeout-`); a run that saved
/// none passed when it exited 0 and failed otherwise.
pub fn ended(captured: &Captured) -> Ended {
    let output = captured.stdout.clone();
    let artifact = output.lines().rev().find_map(|line| {
        line.split_once("Test unit written to ")
            .map(|(_, path)| PathBuf::from(path.trim()))
    });
    let saved = artifact
        .as_ref()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned());
    let status = match saved {
        Some(name) if name.starts_with("timeout-") => FuzzStatus::TimedOut,
        Some(_) => FuzzStatus::Crashed,
        None if captured.exit_code == Some(0) => FuzzStatus::Passed,
        None => FuzzStatus::Failed,
    };
    Ended {
        status,
        executions: stat(&output, "stat::number_of_executed_units:").or_else(|| done(&output)),
        coverage: last_count(&output, " cov: "),
        artifact,
        output,
    }
}

/// The fuzzer's or a sanitizer's own account of a failure (`timeout after 10 seconds`, `deadly
/// signal`, `AddressSanitizer: heap-buffer-overflow on address 0x…`), with addresses masked,
/// for a target whose language reported nothing more specific.
pub fn error(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (_, what) = line.split_once("ERROR: ")?;
        let what = what.trim();
        let what = what.strip_prefix("libFuzzer: ").unwrap_or(what);
        // The seconds a hang ran vary with the machine's load; the limit is the fact.
        if what.starts_with("timeout after ") {
            return Some("timeout: an input ran past the per-input limit".to_owned());
        }
        Some(masked(what))
    })
}

/// `found`, the failure a target's own language reported, else the fuzzer's or a sanitizer's
/// account of it ([`error`]), placed nowhere.
pub fn reported(
    found: Option<(String, Option<Location>)>,
    output: &str,
) -> (String, Option<Location>) {
    found.unwrap_or_else(|| {
        let message = error(output).unwrap_or_else(|| "the target crashed".to_owned());
        (message, None)
    })
}

/// `text` with every hexadecimal address (`0x` and hex digits) as `0x…`.
pub fn masked(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("0x") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        let digits = after
            .find(|c: char| !c.is_ascii_hexdigit())
            .unwrap_or(after.len());
        out.push_str(if digits == 0 { "0x" } else { "0x…" });
        rest = &after[digits..];
    }
    out.push_str(rest);
    out
}

/// The number after `name` on its line.
fn stat(output: &str, name: &str) -> Option<u64> {
    output
        .lines()
        .find_map(|line| line.strip_prefix(name)?.trim().parse().ok())
}

/// The runs of `Done N runs in S second(s)`.
fn done(output: &str) -> Option<u64> {
    output.lines().find_map(|line| {
        line.strip_prefix("Done ")?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    })
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;

    use super::*;

    fn captured(tool: &str, case: &str, exit: i32) -> (Ended, Capture) {
        let capture = Capture::of(&format!("fuzz/{tool}"), case);
        let ended = ended(&Captured {
            exit_code: Some(exit),
            stdout: capture.file("output.txt"),
            stderr: String::new(),
        });
        (ended, capture)
    }

    #[test]
    fn a_saved_crash_input_is_a_crash_with_its_executions() {
        let (ended, capture) = captured("cargo-fuzz", "crash", 77);
        assert_eq!(ended.status, FuzzStatus::Crashed);
        assert_eq!(
            ended.artifact,
            Some(
                capture
                    .ran_in
                    .join(".c/first/inputs/crash-da39a3ee5e6b4b0d3255bfef95601890afd80709")
            )
        );
        assert_eq!((ended.executions, ended.coverage), (Some(1), None));
        assert_eq!(error(&ended.output).as_deref(), Some("deadly signal"));
    }

    #[test]
    fn a_sanitizer_error_is_its_own_account_with_addresses_masked() {
        let output = "==12== ERROR: AddressSanitizer: heap-buffer-overflow on address \
                      0x6020000000f1 at pc 0x10\n";
        assert_eq!(
            error(output).as_deref(),
            Some("AddressSanitizer: heap-buffer-overflow on address 0x… at pc 0x…")
        );
        assert_eq!(masked("0x and 0xg"), "0x and 0xg");
    }

    #[test]
    fn a_saved_timeout_input_is_a_hang() {
        let (ended, _) = captured("cargo-fuzz", "timeout", 70);
        assert_eq!(ended.status, FuzzStatus::TimedOut);
        assert_eq!(
            error(&ended.output).as_deref(),
            Some("timeout: an input ran past the per-input limit")
        );
        assert_eq!(ended.executions, Some(1));
    }

    #[test]
    fn a_run_that_saved_nothing_passed_with_its_coverage() {
        let (ended, _) = captured("jazzer", "pass", 0);
        assert_eq!(ended.status, FuzzStatus::Passed);
        assert_eq!(ended.artifact, None);
        assert_eq!((ended.executions, ended.coverage), (Some(2000), Some(2)));
    }

    #[test]
    fn a_run_that_saved_nothing_and_exited_otherwise_failed() {
        let ended = ended(&Captured {
            exit_code: Some(1),
            stdout: "error: could not compile `units`\n".to_owned(),
            stderr: String::new(),
        });
        assert_eq!(ended.status, FuzzStatus::Failed);
        assert_eq!(ended.executions, None);
    }

    #[test]
    fn the_flags_hand_over_the_budget_and_keep_inputs_in_the_area() {
        let area = Path::new("/cache/first");
        assert_eq!(
            flags(
                &Budget {
                    cases: 100,
                    seed: 0
                },
                area
            ),
            [
                "-runs=100",
                "-seed=1",
                "-timeout=10",
                "-max_total_time=300",
                "-artifact_prefix=/cache/first/inputs/",
                "-print_final_stats=1",
            ]
        );
        let committed = tempfile::tempdir().unwrap();
        assert_eq!(
            corpora(area, Some(committed.path().to_path_buf())),
            [
                "/cache/first/corpus".to_owned(),
                committed.path().to_string_lossy().into_owned()
            ]
        );
        assert_eq!(
            corpora(area, Some(PathBuf::from("/no/such"))),
            ["/cache/first/corpus"]
        );
    }
}
