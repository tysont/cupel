// ABOUTME: Reads a check's `[checks."<id>"]` settings (per-language override already merged) against
// ABOUTME: the settings it declares, rejecting unknown keys and values of the wrong shape.
use std::collections::BTreeMap;

use cupel_core::config::{ConfigError, Table};
use cupel_core::{CheckCtx, CheckId, Manifest, Plan, Result};
use serde_json::Value;

/// Non-negative integer settings, each name with its default.
pub struct Limits {
    defaults: &'static [(&'static str, u32)],
}

impl Limits {
    pub const fn new(defaults: &'static [(&'static str, u32)]) -> Self {
        Self { defaults }
    }

    /// `plan`, once the check `manifest` describes has only valid settings in `ctx`
    /// ([`read`](Self::read)); else the configuration error.
    pub fn plan(&self, manifest: &Manifest, ctx: &CheckCtx, plan: Plan) -> Result<Plan> {
        self.read(&manifest.id, &ctx.config).map(|_| plan)
    }

    /// The value of every setting, configured or default, by name. Any key that is not a
    /// setting, and any setting that is not a non-negative integer, is a configuration error.
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

    /// Every setting name, in declaration order.
    fn names(&self) -> impl Iterator<Item = &'static str> {
        self.defaults.iter().map(|(name, _)| *name)
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
