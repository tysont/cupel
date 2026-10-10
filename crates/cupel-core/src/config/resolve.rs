// ABOUTME: Finds `cupel.toml`, then layers flags > `CUPEL_*` env > profile > file > defaults into the
// ABOUTME: effective configuration, and echoes it with its sources for the envelope's `run.config`.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::config::ConfigError;
use crate::config::file::{CheckConfig, ConfigFile, GateConfig, OracleConfig, RootConfig, Table};
use crate::config::settings::{Format, PROFILE_ENV_VAR, Settings, TimeLimit};
use crate::envelope::{ConfigEcho, ConfigSource};
use crate::error::Result;
use crate::grade::CostClass;
use crate::id::{CheckId, LangId};

/// The configuration file's name.
pub const FILE_NAME: &str = "cupel.toml";

/// What the CLI collects before configuration is resolved.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigRequest {
    /// The repository root (`--cwd`). Discovery starts here and relative paths resolve
    /// against it; core never consults the process's working directory.
    pub root: PathBuf,
    /// `--config PATH`.
    pub config_path: Option<PathBuf>,
    /// `--profile NAME`.
    pub profile: Option<String>,
    /// Settings given as flags.
    pub flags: Settings,
    /// The process environment. Only `CUPEL_*` variables are read.
    pub env: BTreeMap<String, String>,
}

/// The resolved value of every layered setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Effective {
    pub depth: CostClass,
    /// Null means auto-detect: text on a terminal, JSON otherwise (design §6.1).
    pub format: Option<Format>,
    pub budget: Option<TimeLimit>,
    /// The selected profile.
    pub profile: Option<String>,
}

impl Effective {
    const DEFAULT_DEPTH: CostClass = CostClass::Standard;
}

/// A resolved configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Where the file was read from; `None` when no `cupel.toml` was found.
    pub path: Option<PathBuf>,
    pub file: ConfigFile,
    pub effective: Effective,
    /// The layers that contributed, in precedence order (ADR-017).
    pub sources: Vec<ConfigSource>,
}

/// The `--config` path resolved against `root`, or else the nearest `cupel.toml` in `root` or
/// one of its ancestors. `None` when there is none.
pub fn discover(root: &Path, explicit: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(explicit) = explicit {
        let path = root.join(explicit);
        return if path.is_file() {
            Ok(Some(path))
        } else {
            Err(ConfigError::Missing { path }.into())
        };
    }
    Ok(root
        .ancestors()
        .map(|dir| dir.join(FILE_NAME))
        .find(|path| path.is_file()))
}

impl Config {
    /// Discovers, reads, parses, and resolves the configuration for `request`.
    pub fn load(request: &ConfigRequest) -> Result<Self> {
        let Some(path) = discover(&request.root, request.config_path.as_deref())? else {
            return Self::resolve(None, ConfigFile::default(), request);
        };
        let text = fs::read_to_string(&path).map_err(|err| ConfigError::Unreadable {
            path: path.clone(),
            reason: err.to_string(),
        })?;
        let file = ConfigFile::parse(&text, &path)?;
        Self::resolve(Some(path), file, request)
    }

    /// Layers `request` over `file`. `path` is where `file` was read from; pass `None` with an
    /// empty file when no `cupel.toml` exists.
    pub fn resolve(
        path: Option<PathBuf>,
        file: ConfigFile,
        request: &ConfigRequest,
    ) -> Result<Self> {
        let env = Settings::from_env(&request.env)?;
        let profile_name = request
            .profile
            .clone()
            .or_else(|| request.env.get(PROFILE_ENV_VAR).cloned());
        let profile = match &profile_name {
            None => None,
            Some(name) => Some(file.profile.get(name).cloned().ok_or_else(|| {
                ConfigError::UnknownProfile {
                    name: name.clone(),
                    defined: file.profile.keys().cloned().collect(),
                    file_found: path.is_some(),
                }
            })?),
        };
        let file_settings = file.settings();
        let layers = [
            &request.flags,
            &env,
            profile.as_ref().unwrap_or(&Settings::default()),
            &file_settings,
        ]
        .into_iter()
        .fold(Settings::default(), Settings::or);

        let mut sources = Vec::new();
        if !request.flags.is_empty() {
            sources.push(ConfigSource::Flags);
        }
        if !env.is_empty() {
            sources.push(ConfigSource::Env);
        }
        if profile.is_some() {
            sources.push(ConfigSource::Profile);
        }
        if path.is_some() {
            sources.push(ConfigSource::CupelToml);
        }
        if layers.depth.is_none() || layers.format.is_none() || layers.budget.is_none() {
            sources.push(ConfigSource::Defaults);
        }

        Ok(Self {
            path,
            file,
            effective: Effective {
                depth: layers.depth.unwrap_or(Effective::DEFAULT_DEPTH),
                format: layers.format,
                budget: layers.budget,
                profile: profile_name,
            },
            sources,
        })
    }

    /// The envelope's `run.config`.
    pub fn echo(&self) -> ConfigEcho {
        #[derive(Serialize)]
        struct EffectiveEcho<'a> {
            #[serde(flatten)]
            settings: &'a Effective,
            required_version: &'a Option<String>,
            default_branch: &'a Option<String>,
            roots: &'a BTreeMap<String, RootConfig>,
            lang: &'a BTreeMap<LangId, Table>,
            checks: &'a BTreeMap<CheckId, CheckConfig>,
            oracles: &'a BTreeMap<String, OracleConfig>,
            gate: &'a GateConfig,
        }

        let echo = EffectiveEcho {
            settings: &self.effective,
            required_version: &self.file.required_version,
            default_branch: &self.file.default_branch,
            roots: &self.file.roots,
            lang: &self.file.lang,
            checks: &self.file.checks,
            oracles: &self.file.oracles,
            gate: &self.file.gate,
        };
        let Ok(Value::Object(effective)) = serde_json::to_value(echo) else {
            unreachable!("the echo is a struct of serializable maps and scalars")
        };
        ConfigEcho {
            effective,
            sources: self.sources.clone(),
        }
    }
}
