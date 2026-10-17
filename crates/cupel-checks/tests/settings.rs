// ABOUTME: Every native check's declared settings (ADR-111) are exactly the keys its validation accepts: an
// ABOUTME: unknown key's error lists the declared names, and each declared default is itself a valid value.
mod support;

use cupel_core::{Check, Error, Scope, SettingSpec};
use serde_json::{Map, Value, json};

/// Every registered check with each of its settings.
fn declared() -> Vec<(&'static dyn Check, &'static SettingSpec)> {
    support::registry()
        .iter()
        .map(|check| support::check(check.manifest().id.as_str()))
        .flat_map(|check| {
            check
                .manifest()
                .settings
                .iter()
                .map(move |setting| (check, setting))
        })
        .collect()
}

#[test]
fn an_unknown_setting_is_refused_with_exactly_the_declared_names() {
    let checks: Vec<&dyn Check> = support::registry()
        .iter()
        .map(|check| support::check(check.manifest().id.as_str()))
        .collect();
    let refused: Vec<(String, (String, Vec<String>))> = checks
        .iter()
        .map(|check| {
            let error = support::setting_error(*check, json!({"not_a_setting": 1}));
            (check.manifest().id.to_string(), error)
        })
        .collect();
    let expected: Vec<(String, (String, Vec<String>))> = checks
        .iter()
        .map(|check| {
            let names = check.manifest().settings.iter().map(|s| s.name.clone());
            let error = ("not_a_setting".to_owned(), names.collect());
            (check.manifest().id.to_string(), error)
        })
        .collect();
    assert_eq!(refused, expected);
}

#[test]
fn every_declared_default_is_accepted_as_a_configured_value() {
    let refused: Vec<String> = declared()
        .into_iter()
        .filter(|(_, setting)| !setting.default.is_null())
        .filter(|(check, setting)| {
            let config = Map::from_iter([(setting.name.clone(), setting.default.clone())]);
            let (ctx, _dir) = support::ctx(Scope::default(), Value::Object(config));
            matches!(check.plan(&ctx, &Scope::default()), Err(Error::Config(_)))
        })
        .map(|(check, setting)| format!("{}.{}", check.manifest().id, setting.name))
        .collect();
    assert_eq!(refused, Vec::<String>::new());
}

#[test]
fn every_setting_has_a_summary() {
    let unsummarized: Vec<String> = declared()
        .into_iter()
        .filter(|(_, setting)| setting.summary.is_empty())
        .map(|(check, setting)| format!("{}.{}", check.manifest().id, setting.name))
        .collect();
    assert_eq!(unsummarized, Vec::<String>::new());
}
