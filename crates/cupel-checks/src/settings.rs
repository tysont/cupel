// ABOUTME: Reads a check's `[checks."<id>"]` settings (per-language override already merged) against
// ABOUTME: the settings it declares, rejecting unknown keys and values of the wrong shape.
use std::collections::BTreeMap;

use cupel_core::config::{ConfigError, Table};
use cupel_core::{CheckCtx, CheckId, FactKind, Manifest, Plan, Result, Scope, SettingSpec};

use crate::manifest::setting;
use crate::outcome;
use serde_json::Value;

/// Non-negative integer settings, each name with its default and what it sets, and boolean
/// settings after them.
pub struct Limits {
    defaults: &'static [(&'static str, u32, &'static str)],
    flags: &'static [Flag],
}

/// A boolean setting: its name, its default, what it sets, and the fact whose production it
/// configures, if any (ADR-147).
pub struct Flag {
    pub name: &'static str,
    pub default: bool,
    pub summary: &'static str,
    pub configures: Option<FactKind>,
}

impl Limits {
    pub const fn new(defaults: &'static [(&'static str, u32, &'static str)]) -> Self {
        Self {
            defaults,
            flags: &[],
        }
    }

    /// These settings with `flags` declared after them.
    pub const fn with_flags(self, flags: &'static [Flag]) -> Self {
        Self {
            defaults: self.defaults,
            flags,
        }
    }

    /// Every setting as the manifest declares it, in declaration order (ADR-111).
    pub fn specs(&self) -> Vec<SettingSpec> {
        let limits = self
            .defaults
            .iter()
            .map(|(name, default, summary)| setting(name, Value::from(*default), summary));
        let flags = self.flags.iter().map(|flag| SettingSpec {
            configures: flag.configures,
            ..setting(flag.name, Value::Bool(flag.default), flag.summary)
        });
        limits.chain(flags).collect()
    }

    /// `plan`, once the check `manifest` describes has only valid settings in `ctx`
    /// ([`read`](Self::read)); else the configuration error.
    pub fn plan(&self, manifest: &Manifest, ctx: &CheckCtx, plan: Plan) -> Result<Plan> {
        self.read(&manifest.id, &ctx.config).map(|_| plan)
    }

    /// A plan over every file in `scope` ([`outcome::plan_files`]), once the settings in `ctx`
    /// are valid.
    pub fn plan_files(&self, manifest: &Manifest, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        self.plan(manifest, ctx, outcome::plan_files(manifest, scope))
    }

    /// A plan over every symbol in `scope` ([`outcome::plan_symbols`]), once the settings in
    /// `ctx` are valid.
    pub fn plan_symbols(&self, manifest: &Manifest, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        self.plan(manifest, ctx, outcome::plan_symbols(manifest, scope))
    }

    /// The value of every integer setting, configured or default, by name. Any key that is not a
    /// setting, any integer setting that is not a non-negative integer, and any boolean setting
    /// that is not a boolean is a configuration error.
    pub fn read(&self, check: &CheckId, config: &Table) -> Result<BTreeMap<&'static str, u32>> {
        let error = self.error(check);
        if let Some(unknown) = config
            .keys()
            .find(|key| !self.names().any(|name| name == key.as_str()))
        {
            return Err(error(unknown, "not a setting of this check".to_owned()).into());
        }
        if let Some((flag, other)) =
            self.flags
                .iter()
                .find_map(|flag| match config.get(flag.name) {
                    None | Some(Value::Bool(_)) => None,
                    Some(other) => Some((flag.name, other)),
                })
        {
            return Err(error(flag, format!("expected true or false, got {other}")).into());
        }
        self.defaults
            .iter()
            .map(|(name, default, _)| match config.get(*name) {
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
        self.defaults
            .iter()
            .map(|(name, _, _)| *name)
            .chain(self.flags.iter().map(|flag| flag.name))
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
