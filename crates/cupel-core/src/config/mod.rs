// ABOUTME: The `cupel.toml` configuration model, its discovery, and layered resolution (design §6.7).
// ABOUTME: Produces the effective settings, per-check config slices, typed gate rules, and the `run.config` echo.
mod file;
mod gate;
mod registered;
mod resolve;
mod settings;

pub use file::{
    CheckConfig, ConfigFile, GateConfig, GateSection, OracleConfig, OracleKind, RootConfig, Table,
};
pub use gate::{Comparison, GateCondition, GateKey, GateRuleSpec, Threshold};
pub use resolve::{Config, ConfigRequest, Effective, FILE_NAME, discover};
pub use settings::{Format, PROFILE_ENV_VAR, Setting, Settings, TimeLimit};

use std::path::PathBuf;

/// Renders a list of names as `` `a`, `b` ``, or `none`.
fn quoted_list(names: &[String]) -> String {
    if names.is_empty() {
        return "none".to_owned();
    }
    names
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn profiles_hint(defined: &[String], file_found: bool) -> String {
    if file_found {
        quoted_list(defined)
    } else {
        "none (no cupel.toml was found)".to_owned()
    }
}

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
    /// A `CUPEL_*` variable holds a value its setting does not accept.
    #[error("invalid value `{value}` for {var}: {reason}")]
    InvalidEnv {
        var: &'static str,
        value: String,
        reason: String,
    },
    /// The file could not be parsed or has unknown keys; the message lists the valid keys.
    #[error("invalid configuration in `{}`: {message}", path.display())]
    Invalid { path: PathBuf, message: String },
    /// `--config` names a file that does not exist.
    #[error("config file `{}` does not exist; fix the --config path", path.display())]
    Missing { path: PathBuf },
    /// The file exists but could not be read.
    #[error("cannot read config file `{}`: {reason}", path.display())]
    Unreadable { path: PathBuf, reason: String },
    /// `--profile` or `CUPEL_PROFILE` names a profile the file does not define.
    #[error(
        "profile `{name}` is not defined; defined profiles: {}",
        profiles_hint(defined, *file_found)
    )]
    UnknownProfile {
        name: String,
        defined: Vec<String>,
        file_found: bool,
    },
    /// A check rejected a setting in its `[checks."<id>"]` table (or a per-language override).
    #[error("invalid setting `{key}` in `[checks.\"{check}\"]`: {reason}")]
    CheckSetting {
        check: String,
        key: String,
        reason: String,
        /// The settings the check accepts.
        valid: Vec<String>,
    },
    /// An ID is well formed but no registry knows it (ADR-017).
    #[error(
        "unknown {what} in `{section}`; registered: {}",
        quoted_list(registered)
    )]
    Unregistered {
        /// What is unknown, such as ``check `tests.mutaton` ``.
        what: String,
        /// The section naming it, such as `[gate.changed]`.
        section: String,
        registered: Vec<String>,
    },
}
