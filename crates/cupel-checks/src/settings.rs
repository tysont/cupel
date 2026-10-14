// ABOUTME: Reads a check's `[checks."<id>"]` settings (per-language override already merged) against
// ABOUTME: the settings it declares, rejecting unknown keys and values of the wrong shape.
use std::collections::BTreeMap;

use cupel_core::config::{ConfigError, Table};
use cupel_core::{CheckId, Result};
use serde_json::Value;

/// Non-negative integer settings: each name with its default.
pub struct Limits {
    defaults: &'static [(&'static str, u32)],
}

impl Limits {
    pub const fn new(defaults: &'static [(&'static str, u32)]) -> Self {
        Self { defaults }
    }

    /// The value of every setting, configured or default, by name. Any other key, and any
    /// value that is not a non-negative integer, is a configuration error.
    pub fn read(&self, check: &CheckId, config: &Table) -> Result<BTreeMap<&'static str, u32>> {
        let valid: Vec<String> = self
            .defaults
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect();
        let error = |key: &str, reason: String| ConfigError::CheckSetting {
            check: check.to_string(),
            key: key.to_owned(),
            reason,
            valid: valid.clone(),
        };
        if let Some(unknown) = config.keys().find(|key| !valid.contains(key)) {
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
}
