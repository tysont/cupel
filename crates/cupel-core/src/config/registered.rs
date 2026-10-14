// ABOUTME: Validates a resolved config against the check and language registries (ADR-017).
// ABOUTME: Parsing checks ID syntax only; this is where unknown checks, metrics, and languages are caught.
use crate::check::{Check, CheckRegistry, Manifest};
use crate::config::Config;
use crate::config::error::ConfigError;
use crate::config::file::GateSection;
use crate::error::Result;
use crate::id::{CheckId, LangId};
use crate::rank::MetricPer;

impl Config {
    /// Rejects check IDs, gate metrics, and language IDs that the registries do not know. The
    /// CLI calls this once checks register (P2.1); until then it calls `validate_languages`.
    pub fn validate_registered(&self, checks: &CheckRegistry, langs: &[LangId]) -> Result<()> {
        self.validate_languages(langs)?;
        let registry = Registries { checks, langs };
        for id in self.file.checks.keys() {
            registry.check(id, "[checks]")?;
        }
        let gates = [
            ("[gate.changed]", &self.file.gate.changed),
            ("[gate.tree]", &self.file.gate.tree),
        ];
        for (section, gate) in gates {
            gate.iter()
                .flat_map(GateSection::rules)
                .try_for_each(|rule| {
                    let manifest = registry.check(&rule.key.check, section)?;
                    if section == "[gate.tree]" && needs_range(checks, &rule.key.check) {
                        return Err(ConfigError::TreeGateNeedsRange {
                            check: rule.key.check.to_string(),
                        }
                        .into());
                    }
                    match &rule.key.metric {
                        Some(metric) => declared_metric(manifest, metric, section),
                        None => Ok(()),
                    }
                })?;
        }
        Ok(())
    }

    /// Rejects language IDs in `[lang]`, `[roots]`, and per-check `lang` overrides that `langs`
    /// does not hold, leaving check IDs and gate keys alone: the language half of
    /// `validate_registered`, usable before any check is registered (ADR-038).
    pub fn validate_languages(&self, langs: &[LangId]) -> Result<()> {
        let checks = CheckRegistry::default();
        let registry = Registries {
            checks: &checks,
            langs,
        };
        for lang in self.file.lang.keys() {
            registry.lang(lang, "[lang]")?;
        }
        for (name, root) in &self.file.roots {
            registry.lang(&root.lang, &format!("[roots.{name}]"))?;
        }
        for (id, check) in &self.file.checks {
            for lang in check.lang.keys() {
                registry.lang(lang, &format!("[checks.\"{id}\".lang]"))?;
            }
        }
        Ok(())
    }
}

struct Registries<'a> {
    checks: &'a CheckRegistry,
    langs: &'a [LangId],
}

impl<'a> Registries<'a> {
    fn check(&self, id: &CheckId, section: &str) -> Result<&'a Manifest> {
        self.checks.get(id).map(Check::manifest).ok_or_else(|| {
            unregistered(
                format!("check `{id}`"),
                section,
                self.checks
                    .iter()
                    .map(|check| check.manifest().id.to_string()),
            )
        })
    }

    fn lang(&self, lang: &LangId, section: &str) -> Result<()> {
        if self.langs.contains(lang) {
            return Ok(());
        }
        Err(unregistered(
            format!("language `{lang}`"),
            section,
            self.langs.iter().map(ToString::to_string),
        ))
    }
}

/// Whether the registered check `id` reads a range's base side (ADR-056).
fn needs_range(checks: &CheckRegistry, id: &CheckId) -> bool {
    checks
        .get(id)
        .is_some_and(|check| !check.base_needs().is_empty())
}

/// Rejects a gate metric the check's manifest does not declare as a `scope` metric: per-target
/// metrics rank targets and have no one value to compare (ADR-060 §1).
fn declared_metric(manifest: &Manifest, metric: &str, section: &str) -> Result<()> {
    let gate_keys = || {
        manifest
            .metrics
            .iter()
            .filter(|spec| spec.per == MetricPer::Scope)
    };
    if gate_keys().any(|spec| spec.name == metric) {
        return Ok(());
    }
    Err(unregistered(
        format!("metric `{metric}` of `{}`", manifest.id),
        section,
        gate_keys().map(|spec| spec.name.clone()),
    ))
}

fn unregistered(
    what: String,
    section: &str,
    registered: impl Iterator<Item = String>,
) -> crate::error::Error {
    let mut registered: Vec<String> = registered.collect();
    registered.sort();
    ConfigError::Unregistered {
        what,
        section: section.to_owned(),
        registered,
    }
    .into()
}
