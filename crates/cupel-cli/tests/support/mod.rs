// ABOUTME: Helpers shared by the CLI integration tests: invoking the binary and reading envelopes.
// ABOUTME: Each test file includes this module with `mod support;`.
#![allow(
    dead_code,
    reason = "each test crate uses a different subset of the helpers"
)]

use assert_cmd::Command;
use cupel_core::{CheckId, Envelope, FindingId};
use serde_json::{Value, json};

/// The `cupel` binary with a clean environment: no `CUPEL_*` variables leak in from the caller.
pub fn cupel() -> Command {
    let mut command = Command::cargo_bin("cupel").expect("cupel binary is built");
    for (key, _) in std::env::vars() {
        if key.starts_with("CUPEL_") {
            command.env_remove(key);
        }
    }
    command
}

/// What one invocation produced.
pub struct Outcome {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Outcome {
    /// Stdout parsed as JSON, after checking it is a well-typed envelope whose `exit_code`
    /// matches the process exit code.
    pub fn envelope(&self) -> Value {
        let value: Value = serde_json::from_str(&self.stdout)
            .unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{}", self.stdout));
        let typed: Envelope = serde_json::from_value(value.clone())
            .unwrap_or_else(|e| panic!("stdout is not an envelope ({e}):\n{}", self.stdout));
        assert_eq!(
            i32::from(typed.exit_code.code()),
            self.code,
            "exit_code field"
        );
        value
    }

    /// The envelope's only finding.
    pub fn finding(&self) -> Value {
        let envelope = self.envelope();
        let findings = envelope["findings"].as_array().expect("findings array");
        assert_eq!(findings.len(), 1, "exactly one finding: {findings:#?}");
        findings[0].clone()
    }
}

pub fn run<I, S>(args: I) -> Outcome
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    run_with(&mut cupel(), args)
}

/// Runs `command` with `args` in a fresh temporary working directory, so a verb that keeps a run
/// record under `.cupel/` never writes into the source tree; tests that inspect what a run
/// wrote pass `--cwd` explicitly.
pub fn run_with<I, S>(command: &mut Command, args: I) -> Outcome
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let dir = tempfile::tempdir().expect("temporary directory");
    let output = command
        .args(args)
        .current_dir(dir.path())
        .output()
        .expect("cupel runs");
    Outcome {
        code: output.status.code().expect("exit code"),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    }
}

/// The envelope with run-specific values replaced, for snapshots.
pub fn redacted(mut envelope: Value) -> Value {
    envelope["run"]["id"] = json!("[run id]");
    envelope["run"]["started"] = json!("[started]");
    envelope["run"]["duration_ms"] = json!(0);
    envelope["run"]["seed"] = json!("[seed]");
    envelope["cupel"]["version"] = json!("[version]");
    if let Some(description) = envelope["description"].as_object_mut()
        && description.contains_key("version")
    {
        description.insert("version".into(), json!("[version]"));
    }
    envelope
}

/// Text output with run-specific values replaced, for snapshots: the `run` line (ID, start time,
/// duration) and the version.
pub fn redacted_text(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.starts_with("run       run_") {
                "run       [run id, start, duration, seed]".to_owned()
            } else {
                line.replace(env!("CARGO_PKG_VERSION"), "[version]")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// The ID ADR-016 assigns a CLI finding about `subject`.
pub fn cli_finding_id(kind: &str, subject: &str) -> String {
    FindingId::for_subject(&CheckId::from("cli"), kind, subject).to_string()
}
