// ABOUTME: Validates a resolved config against the check and language registries (ADR-017).
// ABOUTME: Parsing checks ID syntax only; this is where unknown checks, metrics, and languages are caught.
use crate::check::{Check, CheckRegistry, Manifest};
use crate::config::file::GateSection;
use crate::config::{Config, ConfigError};
use crate::error::Result;
use crate::id::{CheckId, LangId};

impl Config {
    /// Rejects check IDs, gate metrics, and language IDs that the registries do not know. The
    /// CLI calls this once the registries are populated: checks from P2.1, languages from P1.1.
    pub fn validate_registered(&self, checks: &CheckRegistry, langs: &[LangId]) -> Result<()> {
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
                    match &rule.key.metric {
                        Some(metric) => declared_metric(manifest, metric, section),
                        None => Ok(()),
                    }
                })?;
        }
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

/// Rejects a gate metric the check's manifest does not declare.
fn declared_metric(manifest: &Manifest, metric: &str, section: &str) -> Result<()> {
    if manifest.metrics.iter().any(|spec| spec.name == metric) {
        return Ok(());
    }
    Err(unregistered(
        format!("metric `{metric}` of `{}`", manifest.id),
        section,
        manifest.metrics.iter().map(|spec| spec.name.clone()),
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
