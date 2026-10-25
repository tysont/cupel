// ABOUTME: The layered settings that flags, `CUPEL_*` env, a profile, and `cupel.toml` can all set.
// ABOUTME: Defines each setting's env var once, plus the output format, time-limit, and memory-limit value types.
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::value::StrDeserializer;
use serde::{Deserialize, Serialize};

use crate::config::error::ConfigError;
use crate::error::{Error, Result};
use crate::grade::CostClass;

/// Selects a profile when `--profile` is not given.
pub const PROFILE_ENV_VAR: &str = "CUPEL_PROFILE";

/// A setting every configuration layer can supply (ADR-017).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Setting {
    Depth,
    Format,
    Budget,
}

impl Setting {
    pub const ALL: [Setting; 3] = [Self::Depth, Self::Format, Self::Budget];

    /// The key in `cupel.toml` and `[profile.*]`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Depth => "depth",
            Self::Format => "format",
            Self::Budget => "budget",
        }
    }

    /// The environment variable that sets it.
    pub fn env_var(self) -> &'static str {
        match self {
            Self::Depth => "CUPEL_DEPTH",
            Self::Format => "CUPEL_FORMAT",
            Self::Budget => "CUPEL_BUDGET",
        }
    }
}

/// One layer's settings; unset values fall through to the next layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub depth: Option<CostClass>,
    pub format: Option<Format>,
    pub budget: Option<TimeLimit>,
}

impl Settings {
    /// The settings given by `CUPEL_*` variables in `env`. Other variables are ignored.
    pub fn from_env(env: &BTreeMap<String, String>) -> Result<Self> {
        let var = |setting: Setting| env.get(setting.env_var()).map(String::as_str);
        Ok(Self {
            depth: parse_env(Setting::Depth, var(Setting::Depth))?,
            format: parse_env(Setting::Format, var(Setting::Format))?,
            budget: parse_env(Setting::Budget, var(Setting::Budget))?,
        })
    }

    /// Each setting from `self` where set, else from `lower`.
    #[must_use]
    pub fn or(self, lower: &Settings) -> Settings {
        Settings {
            depth: self.depth.or(lower.depth),
            format: self.format.or(lower.format),
            budget: self.budget.or(lower.budget),
        }
    }

    /// Whether this layer sets nothing.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Parses an env value with the setting's own deserializer, so env accepts exactly what
/// `cupel.toml` accepts.
fn parse_env<T: for<'de> Deserialize<'de>>(
    setting: Setting,
    value: Option<&str>,
) -> Result<Option<T>> {
    let Some(value) = value else {
        return Ok(None);
    };
    T::deserialize(StrDeserializer::<serde::de::value::Error>::new(value))
        .map(Some)
        .map_err(|err| {
            ConfigError::InvalidEnv {
                var: setting.env_var(),
                value: value.to_owned(),
                reason: err.to_string(),
            }
            .into()
        })
}

/// An output format (design §6.3).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    Json,
    Jsonl,
    Text,
    Sarif,
}

impl Format {
    pub const ALL: [Format; 4] = [Self::Json, Self::Jsonl, Self::Text, Self::Sarif];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Jsonl => "jsonl",
            Self::Text => "text",
            Self::Sarif => "sarif",
        }
    }
}

/// A wall-clock limit written as a whole number and a unit: `250ms`, `20s`, `5m`, `1h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TimeLimit {
    amount: u64,
    unit: TimeUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TimeUnit {
    Millis,
    Seconds,
    Minutes,
    Hours,
}

impl TimeUnit {
    /// In match order: `ms` before its one-letter prefix `m`.
    const MATCH_ORDER: [TimeUnit; 4] = [Self::Millis, Self::Seconds, Self::Minutes, Self::Hours];

    fn suffix(self) -> &'static str {
        match self {
            Self::Millis => "ms",
            Self::Seconds => "s",
            Self::Minutes => "m",
            Self::Hours => "h",
        }
    }

    fn duration(self, amount: u64) -> Duration {
        match self {
            Self::Millis => Duration::from_millis(amount),
            Self::Seconds => Duration::from_secs(amount),
            Self::Minutes => Duration::from_secs(amount.saturating_mul(60)),
            Self::Hours => Duration::from_secs(amount.saturating_mul(3600)),
        }
    }
}

impl TimeLimit {
    pub fn as_duration(self) -> Duration {
        self.unit.duration(self.amount)
    }
}

impl fmt::Display for TimeLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.amount, self.unit.suffix())
    }
}

/// `value` as a positive whole number followed by one of `suffixes`, the first in order that
/// ends it: the number and the suffix's index; `None` for any other text.
fn number_with_suffix(value: &str, suffixes: &[&str]) -> Option<(u64, usize)> {
    suffixes.iter().enumerate().find_map(|(index, suffix)| {
        let number = value.strip_suffix(suffix)?;
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let amount: u64 = number.parse().ok().filter(|amount| *amount > 0)?;
        Some((amount, index))
    })
}

impl FromStr for TimeLimit {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let suffixes = TimeUnit::MATCH_ORDER.map(TimeUnit::suffix);
        number_with_suffix(value, &suffixes)
            .map(|(amount, index)| Self {
                amount,
                unit: TimeUnit::MATCH_ORDER[index],
            })
            .ok_or_else(|| Error::InvalidValue {
                what: "duration",
                value: value.to_owned(),
                expected: "a positive whole number followed by `ms`, `s`, `m`, or `h`, \
                           such as `20s`",
            })
    }
}

/// A memory limit written as a whole number and a unit (`512k`, `3m`, `2g`: KiB, MiB, GiB), or
/// as a share of the machine's physical memory (`50%`, from `1%` to `100%`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemoryLimit {
    amount: u64,
    unit: &'static MemoryUnit,
}

/// A unit a memory limit is written in: its suffix, and the power of two it scales by, or
/// `None` for a share of physical memory.
#[derive(Debug, PartialEq, Eq, Hash)]
struct MemoryUnit {
    suffix: &'static str,
    shift: Option<u32>,
}

/// The units, in the order they are matched.
const MEMORY_UNITS: [MemoryUnit; 4] = [
    MemoryUnit {
        suffix: "k",
        shift: Some(10),
    },
    MemoryUnit {
        suffix: "m",
        shift: Some(20),
    },
    MemoryUnit {
        suffix: "g",
        shift: Some(30),
    },
    MemoryUnit {
        suffix: "%",
        shift: None,
    },
];

impl MemoryLimit {
    /// The limit in bytes on a machine with `physical` bytes of memory.
    pub fn resolve(self, physical: u64) -> u64 {
        match self.unit.shift {
            Some(shift) => self.amount.saturating_mul(1 << shift),
            None => u64::try_from(u128::from(physical) * u128::from(self.amount) / 100)
                .unwrap_or(u64::MAX),
        }
    }

    /// The limit in bytes when it is a size; `None` for a share, which needs the machine.
    pub fn bytes(self) -> Option<u64> {
        self.unit.shift.is_some().then(|| self.resolve(0))
    }
}

impl fmt::Display for MemoryLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.amount, self.unit.suffix)
    }
}

impl FromStr for MemoryLimit {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let suffixes = MEMORY_UNITS.each_ref().map(|unit| unit.suffix);
        number_with_suffix(value, &suffixes)
            .map(|(amount, index)| Self {
                amount,
                unit: &MEMORY_UNITS[index],
            })
            // A share is at most the whole.
            .filter(|limit| limit.unit.shift.is_some() || limit.amount <= 100)
            .ok_or_else(|| Error::InvalidValue {
                what: "memory limit",
                value: value.to_owned(),
                expected: "a positive whole number followed by `k`, `m`, or `g`, such as \
                           `512m`, or a share of physical memory from `1%` to `100%`",
            })
    }
}

impl TryFrom<String> for TimeLimit {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        value.parse()
    }
}

impl From<TimeLimit> for String {
    fn from(value: TimeLimit) -> Self {
        value.to_string()
    }
}

impl JsonSchema for TimeLimit {
    fn schema_name() -> Cow<'static, str> {
        "TimeLimit".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": "^[1-9][0-9]*(ms|s|m|h)$",
            "description": "A wall-clock limit: a positive whole number and a unit, such as `20s`."
        })
    }
}
