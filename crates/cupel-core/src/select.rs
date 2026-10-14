// ABOUTME: Check selection by depth, --only/--skip (check IDs or groups), and --tier N+ (design §5.8,
// ABOUTME: §6.4); selected checks keep registry order (ADR-040).
use std::collections::BTreeSet;

use crate::check::{Check, CheckRegistry};
use crate::error::{ArgumentError, Result};
use crate::grade::{CostClass, Tier};

/// The selection flags of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// The effective depth; checks whose cost class is at or below it run by default.
    pub depth: CostClass,
    /// Check IDs or groups; when given, they select regardless of depth.
    pub only: Vec<String>,
    /// Check IDs or groups removed from the selection.
    pub skip: Vec<String>,
    /// `N` or `N+`: only checks of tier N or higher.
    pub tier: Option<String>,
}

impl Selection {
    /// The selected checks, in registry order.
    pub fn select<'a>(&self, registry: &'a CheckRegistry) -> Result<Vec<&'a dyn Check>> {
        let names = Names::of(registry);
        let only = names.resolve("--only", &self.only)?;
        let skip = names.resolve("--skip", &self.skip)?;
        let tier = self.tier.as_deref().map(parse_tier).transpose()?;
        Ok(registry
            .iter()
            .filter(|check| {
                let manifest = check.manifest();
                let id = manifest.id.as_str();
                let chosen = if self.only.is_empty() {
                    manifest.cost <= self.depth
                } else {
                    only.contains(id)
                };
                chosen && !skip.contains(id) && tier.is_none_or(|tier| manifest.tier >= tier)
            })
            .collect())
    }
}

/// Registered check IDs and the groups they form.
struct Names {
    ids: Vec<String>,
}

impl Names {
    fn of(registry: &CheckRegistry) -> Self {
        Self {
            ids: registry
                .iter()
                .map(|check| check.manifest().id.to_string())
                .collect(),
        }
    }

    /// The check IDs `values` name, each a check ID or a group; unknown ones are an error
    /// on `flag`.
    fn resolve(&self, flag: &str, values: &[String]) -> Result<BTreeSet<String>> {
        let mut ids = BTreeSet::new();
        for value in values {
            let named: Vec<&String> = self
                .ids
                .iter()
                .filter(|id| *id == value || group(id) == value)
                .collect();
            if named.is_empty() {
                return Err(ArgumentError::new(
                    flag,
                    Some(value),
                    format!("no check or group named `{value}`"),
                    format!(
                        "Name a registered check or group; `cupel describe` lists them. Valid: {}.",
                        self.valid().join(", ")
                    ),
                )
                .valid(self.valid())
                .into());
            }
            ids.extend(named.into_iter().cloned());
        }
        Ok(ids)
    }

    /// Every group, then every check ID, sorted.
    fn valid(&self) -> Vec<String> {
        let groups: BTreeSet<String> = self.ids.iter().map(|id| group(id).to_owned()).collect();
        let ids: BTreeSet<String> = self.ids.iter().cloned().collect();
        groups.into_iter().chain(ids).collect()
    }
}

/// A check ID's group: its first segment (`static` for `static.complexity`).
fn group(id: &str) -> &str {
    id.split_once('.').map_or(id, |(group, _)| group)
}

fn parse_tier(text: &str) -> Result<Tier> {
    text.strip_suffix('+')
        .unwrap_or(text)
        .parse::<u8>()
        .ok()
        .and_then(|value| Tier::try_from(value).ok())
        .ok_or_else(|| {
            ArgumentError::new(
                "--tier",
                Some(text),
                format!("`--tier {text}` is not a tier"),
                format!(
                    "Pass a tier from 0 to {} with an optional `+`, such as `--tier 3+`.",
                    Tier::MAX
                ),
            )
            .into()
        })
}
