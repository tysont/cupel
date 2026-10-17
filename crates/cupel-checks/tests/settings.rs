// ABOUTME: Every native check's declared settings (ADR-111) are exactly the keys its validation accepts: an
// ABOUTME: unknown key's error lists the declared names, and each declared default is itself a valid value.
mod support;

use cupel_core::{Error, Scope};
use serde_json::{Map, Value, json};

#[test]
fn an_unknown_setting_is_refused_with_exactly_the_declared_names() {
    for check in support::registry().iter() {
        let manifest = check.manifest();
        let declared: Vec<String> = manifest
            .settings
            .iter()
            .map(|setting| setting.name.clone())
            .collect();
        let (key, valid) = support::setting_error(check, json!({"not_a_setting": 1}));
        assert_eq!(
            (key.as_str(), valid),
            ("not_a_setting", declared),
            "{}",
            manifest.id
        );
    }
}

#[test]
fn every_declared_default_is_accepted_as_a_configured_value() {
    for check in support::registry().iter() {
        for setting in &check.manifest().settings {
            if setting.default.is_null() {
                continue;
            }
            let config = Map::from_iter([(setting.name.clone(), setting.default.clone())]);
            let (ctx, _dir) = support::ctx(Scope::default(), Value::Object(config));
            let planned = check.plan(&ctx, &Scope::default());
            assert!(
                !matches!(planned, Err(Error::Config(_))),
                "{} refuses its own default {} = {}: {planned:?}",
                check.manifest().id,
                setting.name,
                setting.default
            );
        }
    }
}

#[test]
fn every_setting_has_a_summary() {
    let unsummarized: Vec<String> = support::registry()
        .iter()
        .flat_map(|check| {
            check
                .manifest()
                .settings
                .iter()
                .filter(|setting| setting.summary.is_empty())
                .map(|setting| format!("{}.{}", check.manifest().id, setting.name))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(unsummarized, Vec::<String>::new());
}
