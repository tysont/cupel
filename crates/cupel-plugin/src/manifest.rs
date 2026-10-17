// ABOUTME: Validates the manifest a plugin's `describe` printed beyond what its type says (ADR-089 §1): ID shapes,
// ABOUTME: declared finding kinds, metrics, and settings, and for a producer its prefix, languages, and provided kind and version.
use std::collections::BTreeSet;

use cupel_core::config::parse_check_id;
use cupel_core::plugin::{ProducerManifest, Provides};
use cupel_core::{LangId, Manifest, schema};

/// Why a check plugin's manifest cannot be registered.
pub(crate) fn check(manifest: &Manifest) -> Result<(), String> {
    let id = &manifest.id;
    parse_check_id(id.as_str()).map_err(|reason| format!("invalid check ID `{id}`: {reason}"))?;
    version(manifest.version)?;
    if manifest.findings.is_empty() {
        return Err("a check plugin must declare at least one finding kind".to_owned());
    }
    let mut kinds = BTreeSet::new();
    for spec in &manifest.findings {
        parse_check_id(&spec.kind)
            .map_err(|_| format!("finding kind `{}` must be `<group>.<name>`", spec.kind))?;
        once(&mut kinds, &spec.kind, "finding kind")?;
    }
    let mut metrics = BTreeSet::new();
    for spec in &manifest.metrics {
        once(&mut metrics, &spec.name, "metric")?;
    }
    let mut settings = BTreeSet::new();
    for spec in &manifest.settings {
        once(&mut settings, &spec.name, "setting")?;
    }
    Ok(())
}

/// Why a producer plugin's manifest cannot be registered, given the registered `languages`.
pub(crate) fn producer(manifest: &ProducerManifest, languages: &[LangId]) -> Result<(), String> {
    let id = manifest.id.as_str();
    if !id.starts_with("plugin.") || id.len() == "plugin.".len() {
        return Err(format!("producer ID `{id}` must start with `plugin.`"));
    }
    version(manifest.version)?;
    if manifest.languages.is_empty() {
        return Err("a producer plugin must name at least one language".to_owned());
    }
    if let Some(unknown) = manifest
        .languages
        .iter()
        .find(|lang| !languages.contains(lang))
    {
        let registered: Vec<String> = languages.iter().map(|lang| format!("`{lang}`")).collect();
        return Err(format!(
            "language `{unknown}` is not registered; registered: {}",
            registered.join(", ")
        ));
    }
    provides(manifest.provides)
}

/// Why a plugin cannot provide `provides`: the kind has no payload type, or another version.
fn provides(provides: Provides) -> Result<(), String> {
    let Provides { kind, version } = provides;
    match schema::fact_version(kind) {
        None => Err(format!("`{kind}` facts have no payload type yet")),
        Some(current) if current != version => Err(format!(
            "provides `{kind}` version {version}, but this Cupel reads version {current}"
        )),
        Some(_) => Ok(()),
    }
}

fn version(version: u32) -> Result<(), String> {
    if version == 0 {
        Err("a manifest's version starts at 1".to_owned())
    } else {
        Ok(())
    }
}

/// Records `name` in `seen`, refusing it the second time.
fn once(seen: &mut BTreeSet<String>, name: &str, what: &str) -> Result<(), String> {
    if seen.insert(name.to_owned()) {
        Ok(())
    } else {
        Err(format!("{what} `{name}` is declared twice"))
    }
}

#[cfg(test)]
mod tests {
    use cupel_core::{CostClass, FactKind, ProducerId, Stability, Tier};
    use serde_json::json;

    use super::*;

    fn manifest() -> Manifest {
        serde_json::from_value(json!({
            "id": "ref.sample", "version": 1, "tier": 2, "cost": "quick", "needs": [], "wants": [],
            "findings": [{"kind": "ref.sample-file", "action": "review", "summary": "", "evidence": {}}],
            "metrics": [{"name": "untested", "summary": "", "per": "scope"}],
            "settings": [{"name": "files_max", "default": 3, "summary": ""}],
            "stability": "experimental", "summary": ""
        }))
        .unwrap()
    }

    fn producer_manifest() -> ProducerManifest {
        ProducerManifest {
            id: ProducerId::from("plugin.sample"),
            version: 1,
            provides: Provides {
                kind: FactKind::PerTestMap,
                version: 2,
            },
            languages: vec![LangId::from("typescript")],
            needs: Vec::new(),
            wants: Vec::new(),
            cost: CostClass::Quick,
            stability: Stability::Experimental,
            summary: String::new(),
        }
    }

    #[test]
    fn a_well_formed_check_manifest_passes() {
        assert_eq!(check(&manifest()), Ok(()));
        assert_eq!(manifest().tier, Tier::try_from(2).unwrap());
    }

    #[test]
    fn check_manifests_are_refused_for_each_rule() {
        let mut zero = manifest();
        zero.version = 0;
        let mut twice = manifest();
        twice.findings.push(twice.findings[0].clone());
        let mut bad_kind = manifest();
        bad_kind.findings[0].kind = "untested".to_owned();
        let mut metrics = manifest();
        metrics.metrics.push(metrics.metrics[0].clone());
        let mut settings = manifest();
        settings.settings.push(settings.settings[0].clone());
        let problems: Vec<String> = [zero, twice, bad_kind, metrics, settings]
            .iter()
            .map(|manifest| check(manifest).unwrap_err())
            .collect();
        assert_eq!(
            problems,
            [
                "a manifest's version starts at 1",
                "finding kind `ref.sample-file` is declared twice",
                "finding kind `untested` must be `<group>.<name>`",
                "metric `untested` is declared twice",
                "setting `files_max` is declared twice",
            ]
        );
    }

    #[test]
    fn producer_manifests_are_refused_for_each_rule() {
        let languages = [LangId::from("typescript")];
        let mut bare = producer_manifest();
        bare.id = ProducerId::from("plugin.");
        let mut none = producer_manifest();
        none.languages.clear();
        let mut zero = producer_manifest();
        zero.version = 0;
        let problems: Vec<String> = [bare, none, zero]
            .iter()
            .map(|manifest| producer(manifest, &languages).unwrap_err())
            .collect();
        assert_eq!(
            problems,
            [
                "producer ID `plugin.` must start with `plugin.`",
                "a producer plugin must name at least one language",
                "a manifest's version starts at 1",
            ]
        );
        assert_eq!(producer(&producer_manifest(), &languages), Ok(()));
    }
}
