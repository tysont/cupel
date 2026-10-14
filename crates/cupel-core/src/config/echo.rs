// ABOUTME: The envelope's `run.config`: the effective configuration and the layers it came from, in
// ABOUTME: precedence order (design §6.7).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The effective configuration and the layers it came from.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ConfigEcho {
    pub effective: Map<String, Value>,
    pub sources: Vec<ConfigSource>,
}

/// A configuration layer, in precedence order (design §6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigSource {
    Flags,
    Env,
    Profile,
    #[serde(rename = "cupel.toml")]
    CupelToml,
    Defaults,
}
