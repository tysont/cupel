// ABOUTME: The error type shared by cupel-core's parsing, registries, and fact access.
// ABOUTME: Every fallible core API returns `cupel_core::Result`.
use crate::config::ConfigError;
use crate::fact::FactKind;
use crate::id::CheckId;
use crate::writer::WriteError;

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
    DuplicateCheck { id: CheckId },
    /// The configuration is invalid; a usage error (exit 2).
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// A file write was refused or failed (ADR-020).
    #[error(transparent)]
    Write(#[from] WriteError),
}

/// Result type for cupel-core APIs.
pub type Result<T, E = Error> = std::result::Result<T, E>;
