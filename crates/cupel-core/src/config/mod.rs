// ABOUTME: The `cupel.toml` configuration model, its discovery, and layered resolution (design §6.7).
// ABOUTME: Produces the effective settings, per-check config slices, typed gate rules, and the `run.config` echo.
mod gate;

pub use gate::{Comparison, GateCondition, GateKey, GateRuleSpec, Threshold};

use std::path::PathBuf;

/// A configuration problem. Every variant is a usage error (exit 2) whose message names the fix.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// A gate rule's key or condition is malformed.
    #[error("invalid gate rule `\"{key}\" = \"{condition}\"`: {reason}")]
    InvalidGateRule {
        key: String,
        condition: String,
        reason: String,
    },
    /// The file could not be parsed or has unknown keys; the message lists the valid keys.
    #[error("invalid configuration in `{}`: {message}", path.display())]
    Invalid { path: PathBuf, message: String },
}
