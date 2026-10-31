// ABOUTME: The error type shared by cupel-core's parsing, registries, and fact access.
// ABOUTME: Every fallible core API returns `cupel_core::Result`.
use std::path::PathBuf;

use crate::config::error::ConfigError;
use crate::fact::FactKind;

/// Errors raised by cupel-core.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// A string did not have the shape an identifier requires.
    #[error("invalid {what} `{value}`: expected {expected}")]
    InvalidId {
        what: &'static str,
        value: String,
        expected: &'static str,
    },
    /// A string value did not have the shape its type requires.
    #[error("invalid {what} `{value}`: expected {expected}")]
    InvalidValue {
        what: &'static str,
        value: String,
        expected: &'static str,
    },
    /// A numeric value was outside the range its type allows.
    #[error("invalid {what} `{value}`: expected {expected}")]
    OutOfRange {
        what: &'static str,
        value: u64,
        expected: &'static str,
    },
    /// A check asked for, or was given, a fact kind its manifest does not declare.
    #[error("fact `{kind}` is not declared in the check's needs or wants")]
    UndeclaredFact { kind: FactKind },
    /// A check asked for a declared fact that was not produced for this run.
    #[error("fact `{kind}` is declared but was not produced")]
    AbsentFact { kind: FactKind },
    /// Two checks were registered under the same ID.
    #[error("check `{id}` is already registered")]
    DuplicateCheck { id: String },
    /// The configuration is invalid; a usage error (exit 2).
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// The run's `Cancellation` fired (`--timeout`); the work stopped early.
    #[error("cancelled: the run's time limit passed")]
    Cancelled,
    /// The run's processes held more memory than the run's budget allows (ADR-148); they
    /// were killed. `held` names each process the budget counted, one per line.
    #[error("stopped: the run held more than {limit} bytes of memory, in:\n{held}")]
    OutOfMemory { limit: u64, held: String },
    /// A file write was refused or failed (ADR-020).
    #[error(transparent)]
    Write(#[from] WriteError),
    /// A command-line value cannot be honored, such as a `--paths` pattern that matches no
    /// file; a usage error (exit 2) naming the fix.
    #[error("{}", .0.title)]
    Argument(Box<ArgumentError>),
    /// An external program could not be started.
    #[error("cannot run `{program}`: {message}")]
    Process { program: String, message: String },
    /// A plugin broke the protocol or failed without reporting (ADR-089 §6).
    #[error("plugin `{plugin}`: {problem}")]
    Plugin { plugin: String, problem: String },
}

/// Why a scope or selection flag cannot be honored, and what to do instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentError {
    /// The flag at fault, such as `--paths`.
    pub argument: String,
    /// The value supplied, when there was one.
    pub value: Option<String>,
    pub title: String,
    /// What to do instead.
    pub fix: String,
    /// The values that would have been accepted, when they are enumerable.
    pub valid: Vec<String>,
}

impl ArgumentError {
    pub fn new(argument: &str, value: Option<&str>, title: String, fix: String) -> Self {
        Self {
            argument: argument.to_owned(),
            value: value.map(str::to_owned),
            title,
            fix,
            valid: Vec::new(),
        }
    }

    #[must_use]
    pub fn valid(mut self, valid: Vec<String>) -> Self {
        self.valid = valid;
        self
    }
}

impl From<ArgumentError> for Error {
    fn from(error: ArgumentError) -> Self {
        Self::Argument(Box::new(error))
    }
}

/// Result type for cupel-core APIs.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Why a write was not made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WriteError {
    /// The path lands outside the writer's area and `--write` was not given.
    #[error("`{}` is outside .cupel/; pass --write to permit writing it", .path.display())]
    Outside { path: PathBuf },
    /// The file system refused.
    #[error("cannot write `{}`: {message}", .path.display())]
    Io { path: PathBuf, message: String },
}
