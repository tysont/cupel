// ABOUTME: The `cupel.toml` configuration model, its discovery, and layered resolution (design §6.7).
// ABOUTME: Produces the effective settings, per-check config slices, typed gate rules, and the `run.config` echo.
mod echo;
pub(crate) mod error;
mod file;
mod gate;
mod registered;
mod resolve;
mod settings;

pub use echo::{ConfigEcho, ConfigSource};
pub use error::ConfigError;
pub use file::{
    CheckConfig, ConfigFile, GateConfig, GateSection, OracleConfig, OracleKind, RootConfig,
    ScopeConfig, Table,
};
pub use gate::{Comparison, GateCondition, GateKey, GateRuleSpec, Threshold};
pub use resolve::{Config, ConfigRequest, Effective, FILE_NAME, discover};
pub use settings::{Format, PROFILE_ENV_VAR, Setting, Settings, TimeLimit};
