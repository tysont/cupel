// ABOUTME: Runs the built `cupel` binary in a chosen directory with a clean `CUPEL_*` environment
// ABOUTME: and captures its exit code, stdout, stderr, and the envelope stdout carries.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cupel_core::Envelope;

/// What one invocation of `cupel` produced.
#[derive(Debug)]
pub struct Outcome {
    /// The process exit code; `None` if a signal ended it.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    /// Stdout parsed as an envelope; `None` when stdout is not a JSON object (text output).
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
}

/// Runs `cupel <args>` in `cwd` and captures everything it produced.
///
/// The binary is the one Cargo built for this workspace (`make test` rebuilds it first). Stdin
/// is closed and no `CUPEL_*` variable reaches it, so the caller's shell cannot change the
/// output.
///
/// # Panics
///
/// When the binary has not been built or cannot be started, and when it prints JSON that is
/// not an envelope, which breaks the output contract.
pub fn run_cupel(args: &[&str], cwd: &Path) -> Outcome {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("cupel"));
    command.args(args).current_dir(cwd).stdin(Stdio::null());
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("CUPEL_") {
            command.env_remove(key);
        }
    }
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("cannot run cupel {args:?} in {}: {error}", cwd.display()));
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let envelope = stdout.trim_start().starts_with('{').then(|| {
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
