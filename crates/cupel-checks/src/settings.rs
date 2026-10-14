// ABOUTME: Reads a check's `[checks."<id>"]` settings (per-language override already merged) against
// ABOUTME: the settings it declares, rejecting unknown keys and values of the wrong shape.
use std::collections::BTreeMap;

use cupel_core::config::{ConfigError, Table};
use cupel_core::{CheckId, Result};
use serde_json::Value;

/// Non-negative integer settings, each name with its default, and boolean flags, which default
/// to false.
pub struct Limits {
    defaults: &'static [(&'static str, u32)],
    flags: &'static [&'static str],
}

impl Limits {
    pub const fn new(defaults: &'static [(&'static str, u32)]) -> Self {
        Self {
            defaults,
            flags: &[],
        }
    }

    /// These settings plus the boolean `flags`.
    pub const fn with_flags(self, flags: &'static [&'static str]) -> Self {
        Self {
            defaults: self.defaults,
            flags,
        }
    }

    /// The value of every integer setting, configured or default, by name. Any key that is
    /// neither a setting nor a flag, and any setting that is not a non-negative integer, is a
    /// configuration error.
    pub fn read(&self, check: &CheckId, config: &Table) -> Result<BTreeMap<&'static str, u32>> {
        let error = self.error(check);
        if let Some(unknown) = config
            .keys()
            .find(|key| !self.names().any(|name| name == key.as_str()))
        {
            return Err(error(unknown, "not a setting of this check".to_owned()).into());
        }
        self.defaults
            .iter()
            .map(|(name, default)| match config.get(*name) {
                None => Ok((*name, *default)),
                Some(Value::Number(number)) => number
                    .as_u64()
                    .and_then(|value| u32::try_from(value).ok())
                    .map(|value| (*name, value))
                    .ok_or_else(|| {
                        error(
                            name,
                            format!("expected a non-negative integer, got {number}"),
                        )
                        .into()
                    }),
                Some(other) => Err(error(
                    name,
                    format!("expected a non-negative integer, got {other}"),
                )
                .into()),
            })
            .collect()
    }

    /// Whether the flag `name` is set; false when absent. A value that is not a boolean is a
    /// configuration error.
    pub fn flag(&self, check: &CheckId, config: &Table, name: &str) -> Result<bool> {
        match config.get(name) {
            None => Ok(false),
            Some(Value::Bool(value)) => Ok(*value),
            Some(other) => {
                Err(self.error(check)(name, format!("expected true or false, got {other}")).into())
            }
        }
    }

    /// Every setting name, then every flag name, in declaration order.
    fn names(&self) -> impl Iterator<Item = &'static str> {
        self.defaults
            .iter()
            .map(|(name, _)| *name)
            .chain(self.flags.iter().copied())
    }

    /// A `CheckSetting` error about a key of `check`, listing every valid name.
    fn error(&self, check: &CheckId) -> impl Fn(&str, String) -> ConfigError {
        let check = check.to_string();
        let valid: Vec<String> = self.names().map(str::to_owned).collect();
        move |key, reason| ConfigError::CheckSetting {
            check: check.clone(),
            key: key.to_owned(),
            reason,
            valid: valid.clone(),
        }
    }
}
