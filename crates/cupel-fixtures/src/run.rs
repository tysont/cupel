// ABOUTME: Runs the built `cupel` binary in a chosen directory with a clean `CUPEL_*` environment and captures
// ABOUTME: its exit code, stdout, stderr, and envelope or SARIF log; under DAEMON_PARITY=1 also proves the daemon prints the same.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cupel_core::Envelope;
use serde_json::Value;

/// What one invocation of `cupel` produced.
#[derive(Debug)]
pub struct Outcome {
    /// The process exit code; `None` if a signal ended it.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    /// Stdout parsed as an envelope; `None` when stdout is not a JSON object (text output) or is
    /// a SARIF log (`--format sarif`).
    pub envelope: Option<Envelope>,
    /// The directory `cupel` ran in, which redaction treats as the repository root.
    pub cwd: PathBuf,
}

impl Outcome {
    /// The envelope stdout carried.
    ///
    /// # Panics
    ///
    /// When stdout carried no envelope; the message shows the exit code, stdout, and stderr.
    pub fn envelope(&self) -> &Envelope {
        self.envelope.as_ref().unwrap_or_else(|| {
            panic!(
                "cupel printed no envelope (exit {:?})\nstdout:\n{}\nstderr:\n{}",
                self.code, self.stdout, self.stderr
            )
        })
    }

    /// Checks this run's envelope against the golden `name` (see [`crate::assert_golden`]).
    #[track_caller]
    pub fn assert_golden(&self, name: &str) {
        crate::assert_golden(name, self.envelope(), &self.cwd);
    }

    /// The SARIF log stdout carried.
    ///
    /// # Panics
    ///
    /// When stdout is not a SARIF log; the message shows the exit code, stdout, and stderr.
    pub fn sarif(&self) -> Value {
        crate::sarif::sarif_log(&self.stdout).unwrap_or_else(|| {
            panic!(
                "cupel printed no SARIF log (exit {:?})\nstdout:\n{}\nstderr:\n{}",
                self.code, self.stdout, self.stderr
            )
        })
    }

    /// Checks this run's SARIF log, redacted by [`crate::redact_sarif`], against the golden
    /// `name`.
    ///
    /// # Panics
    ///
    /// When stdout is not a SARIF log, or it differs from the golden.
    #[track_caller]
    pub fn assert_sarif_golden(&self, name: &str) {
        let actual = crate::redact_sarif(&self.sarif(), &self.cwd);
        if let Err(error) = crate::Goldens::from_env().check(name, &actual) {
            panic!("{error}");
        }
    }
}

/// Runs `cupel <args>` in `cwd` and captures everything it produced.
///
/// The binary is the one Cargo built for this workspace (`make test` rebuilds it first). Stdin
/// is closed and no `CUPEL_*` variable reaches it but `CUPEL_NO_DAEMON=1`, so the caller's shell
/// cannot change the output and the run is cold. With `DAEMON_PARITY=1`, the same invocation
/// also runs through a daemon on a copy of `cwd` made before this run, and a difference after
/// redaction panics (ADR-085 §11).
///
/// # Panics
///
/// When the binary has not been built or cannot be started, and when it prints JSON that is
/// neither an envelope nor a SARIF log, which breaks the output contract.
pub fn run_cupel(args: &[&str], cwd: &Path) -> Outcome {
    run(args, cwd, crate::parity::enabled())
}

/// Runs `cupel <args>` in `cwd` as [`run_cupel`] does, but never through the parity daemon: for
/// an invocation whose output holds wall-time measurements (`perf.bench` comparing benchmark
/// samples, ADR-106), which a second run measures again and cannot print the same. Every other
/// invocation goes through [`run_cupel`].
///
/// # Panics
///
/// As [`run_cupel`].
pub fn run_cupel_measuring(args: &[&str], cwd: &Path) -> Outcome {
    run(args, cwd, false)
}

fn run(args: &[&str], cwd: &Path, parity: bool) -> Outcome {
    let twin = parity.then(|| crate::parity::Twin::of(cwd));
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("cupel"));
    command.args(args).current_dir(cwd).stdin(Stdio::null());
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("CUPEL_") {
            command.env_remove(key);
        }
    }
    command.env("CUPEL_NO_DAEMON", "1");
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("cannot run cupel {args:?} in {}: {error}", cwd.display()));
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if let Some(twin) = twin {
        twin.assert_same(args, (output.status.code(), &stdout, &stderr));
    }
    let is_envelope =
        stdout.trim_start().starts_with('{') && crate::sarif::sarif_log(&stdout).is_none();
    let envelope = is_envelope.then(|| {
        serde_json::from_str(&stdout).unwrap_or_else(|error| {
            panic!("cupel {args:?} printed JSON that is not an envelope ({error}):\n{stdout}")
        })
    });
    Outcome {
        code: output.status.code(),
        stdout,
        stderr,
        envelope,
        cwd: cwd.to_path_buf(),
    }
}
