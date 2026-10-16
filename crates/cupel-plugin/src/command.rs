// ABOUTME: How one plugin runs: its name, program, leading arguments, and the repository root, and one call of
// ABOUTME: a protocol subcommand under the run's cancellation, stopped with SIGTERM before SIGKILL (ADR-089 §7).
use std::path::{Path, PathBuf};
use std::time::Duration;

use cupel_core::plugin::RunRequest;
use cupel_core::{Cancellation, Captured, Error, Process, Result, output_tail};

/// How long a cancelled plugin has to exit after SIGTERM before it is killed.
const GRACE: Duration = Duration::from_secs(2);

/// How long `describe` and `plan` may take (ADR-088 §4, ADR-089 §7).
pub(crate) const QUICK_LIMIT: Duration = Duration::from_secs(10);

/// One plugin's command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginCommand {
    /// The `[plugins]` key, or the executable's file name for a `PATH` plugin.
    pub name: String,
    program: String,
    args: Vec<String>,
    root: PathBuf,
}

impl PluginCommand {
    pub fn new(name: &str, program: &str, args: &[String], root: &Path) -> Self {
        Self {
            name: name.to_owned(),
            program: program.to_owned(),
            args: args.to_vec(),
            root: root.to_path_buf(),
        }
    }

    /// The program and its arguments, as configured, for messages.
    pub fn display(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The program to start: a relative path with a `/` is under the repository root; a bare
    /// name is found on `PATH` (ADR-088 §1).
    pub fn program(&self) -> PathBuf {
        let program = Path::new(&self.program);
        if program.is_relative() && self.program.contains('/') {
            self.root.join(program)
        } else {
            program.to_path_buf()
        }
    }

    /// Runs `<plugin> <subcommand>` in the repository root with `request` on standard input.
    pub fn call(
        &self,
        subcommand: &str,
        request: Option<&RunRequest>,
        cancel: &Cancellation,
    ) -> Result<Captured> {
        let mut process = Process::new(&self.program().to_string_lossy(), &self.root)
            .args(self.args.iter().map(String::as_str))
            .args([subcommand])
            .terminate(GRACE);
        if let Some(request) = request {
            let input = serde_json::to_vec(request).expect("a run request serializes");
            process = process.input(input);
        }
        process.run(cancel)
    }

    /// The error a run of this plugin ends with.
    pub fn error(&self, problem: impl Into<String>) -> Error {
        Error::Plugin {
            plugin: self.name.clone(),
            problem: problem.into(),
        }
    }
}

/// How a call ended that did not succeed, then `context`, then what it wrote to standard error:
/// `exited with code 3 without a result: <stderr>` or `was ended by a signal`.
pub(crate) fn ended(captured: &Captured, context: &str) -> String {
    let how = match captured.exit_code {
        Some(code) => format!("exited with code {code}{context}"),
        None => format!("was ended by a signal{context}"),
    };
    match output_tail(&captured.stderr) {
        Some(stderr) => format!("{how}: {stderr}"),
        None => how,
    }
}
