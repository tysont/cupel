// ABOUTME: Golden envelopes for the registry-rendered `describe` subjects. Each check's description is
// ABOUTME: its own golden; whole-registry goldens keep what no one check or tool producer owns (ADR-074).
use std::collections::BTreeSet;

use cupel_core::{FactKind, LangId, ProducerRegistry, SupportDescription};
use cupel_fixtures::matrix::Registries;
use cupel_fixtures::{Goldens, redact, run_cupel};
use serde_json::{Value, json};

/// The redacted `description` envelope of `cupel <args> --format json`, which must pass.
fn described(args: &[&str]) -> Value {
    let dir = tempfile::tempdir().unwrap();
    let mut args = args.to_vec();
    args.extend(["--format", "json"]);
    let outcome = run_cupel(&args, dir.path());
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    redact(outcome.envelope(), &outcome.cwd)
}

/// Removes `keys` from the envelope's description and returns their values, in order.
fn take(envelope: &mut Value, keys: &[&str]) -> Vec<Value> {
    let description = envelope["description"].as_object_mut().unwrap();
    keys.iter()
        .map(|key| description.remove(*key).unwrap_or(Value::Null))
        .collect()
}

#[track_caller]
fn assert_golden(name: &str, actual: &Value) {
    if let Err(error) = Goldens::from_env().check(name, actual) {
        panic!("{error}");
    }
}

/// The producers the language layer alone registers: what a language's golden pins. Tool
/// producers register for languages too, but each pins its own behavior, so the language
/// goldens do not change when one is added.
fn language_layer(registries: &Registries) -> ProducerRegistry {
    let mut producers = ProducerRegistry::default();
    registries.languages.register_producers(&mut producers);
    producers
}

/// Each fact kind with producers for `lang`, and those producers' IDs, in catalog order.
fn facts_of(producers: &ProducerRegistry, registries: &Registries, lang: &LangId) -> Value {
    let language = registries.languages.get(lang).unwrap();
    let lang = cupel_lang::lang(language.as_ref());
    let facts: Vec<Value> = FactKind::ALL
        .into_iter()
        .filter_map(|kind| {
            let ids: Vec<String> = producers
                .producers_for(kind, &lang)
                .map(|producer| producer.id().to_string())
                .collect();
            (!ids.is_empty()).then(|| json!({"kind": kind, "producers": ids}))
        })
        .collect();
    Value::Array(facts)
}

/// `facts` (a description's `[{kind, producers: [{id, ..}]}]`) as `[{kind, producers: [id]}]`.
fn fact_ids(facts: &Value) -> Value {
    let facts: Vec<Value> = facts
        .as_array()
        .unwrap()
        .iter()
        .map(|fact| {
            let ids: Vec<&Value> = fact["producers"]
                .as_array()
                .unwrap()
                .iter()
                .map(|producer| &producer["id"])
                .collect();
            json!({"kind": fact["kind"], "producers": ids})
        })
        .collect();
    Value::Array(facts)
}

/// `facts` keeping only the entries and producers the language layer registers.
fn language_layer_facts(facts: &Value, layer: &Value) -> Value {
    let kept: BTreeSet<(String, String)> = layer
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|fact| {
            let kind = fact["kind"].as_str().unwrap().to_owned();
            fact["producers"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |id| (kind.clone(), id.as_str().unwrap().to_owned()))
        })
        .collect();
    let facts: Vec<Value> = facts
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|fact| {
            let kind = fact["kind"].as_str().unwrap();
            let producers: Vec<Value> = fact["producers"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|producer| {
                    kept.contains(&(kind.to_owned(), producer["id"].as_str().unwrap().to_owned()))
                })
                .cloned()
                .collect();
            (!producers.is_empty()).then(|| json!({"kind": kind, "producers": producers}))
        })
        .collect();
    Value::Array(facts)
}

/// Every check's support for `lang`, in check order, as `describe language` lists it.
fn column(registries: &Registries, lang: &LangId) -> Value {
    let cells: Vec<Value> = registries
        .pairs()
        .into_iter()
        .filter(|pair| pair.lang == *lang)
        .map(
            |pair| json!({"check": pair.check, "support": SupportDescription::from(&pair.support)}),
        )
        .collect();
    Value::Array(cells)
}

#[test]
fn describe_check_matches_its_golden_for_every_registered_check() {
    let registries = Registries::builtin();
    let mut checked = 0;
    for check in registries.checks.iter() {
        let id = check.manifest().id.as_str();
        assert_golden(&format!("{id}/describe"), &described(&["describe", id]));
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn describe_languages_matches_its_golden() {
    let registries = Registries::builtin();
    let layer = language_layer(&registries);
    let mut envelope = described(&["describe", "languages"]);
    for language in envelope["description"]["languages"].as_array_mut().unwrap() {
        let id = LangId::from(language["id"].as_str().unwrap());
        let kinds = |producers: &ProducerRegistry| -> Vec<Value> {
            facts_of(producers, &registries, &id)
                .as_array()
                .unwrap()
                .iter()
                .map(|fact| fact["kind"].clone())
                .collect()
        };
        assert_eq!(
            language["facts"],
            json!(kinds(&registries.producers)),
            "{id}"
        );
        language["facts"] = json!(kinds(&layer));
    }
    assert_golden("describe/languages", &envelope);
}

#[test]
fn describe_language_matches_its_golden_for_every_language() {
    let registries = Registries::builtin();
    let layer = language_layer(&registries);
    for id in registries.languages.ids() {
        let mut envelope = described(&["describe", "language", id.as_str()]);
        let [checks, facts] =
            <[Value; 2]>::try_from(take(&mut envelope, &["checks", "facts"])).unwrap();
        assert_eq!(checks, column(&registries, &id), "{id}");
        assert_eq!(
            fact_ids(&facts),
            facts_of(&registries.producers, &registries, &id),
            "{id}"
        );
        envelope["description"]["facts"] =
            language_layer_facts(&facts, &facts_of(&layer, &registries, &id));
        assert_golden(&format!("describe/language-{id}"), &envelope);
    }
}

#[test]
fn describe_fact_symbols_matches_its_golden() {
    let registries = Registries::builtin();
    let mut envelope = described(&["describe", "fact", "symbols"]);
    let [consumers] = <[Value; 1]>::try_from(take(&mut envelope, &["consumers"])).unwrap();
    let expected: Vec<String> = registries
        .checks
        .iter()
        .map(cupel_core::Check::manifest)
        .filter(|manifest| {
            let kind = FactKind::Symbols;
            manifest.needs.contains(&kind) || manifest.wants.contains(&kind)
        })
        .map(|manifest| manifest.id.to_string())
        .collect();
    assert_eq!(consumers, json!(expected));
    assert_golden("describe/fact-symbols", &envelope);
}

#[test]
fn describe_matrix_matches_its_golden_and_every_check_s_row() {
    let registries = Registries::builtin();
    let mut envelope = described(&["describe", "matrix"]);
    let [checks, cells] =
        <[Value; 2]>::try_from(take(&mut envelope, &["checks", "cells"])).unwrap();
    let ids: Vec<String> = registries
        .checks
        .iter()
        .map(|check| check.manifest().id.to_string())
        .collect();
    assert_eq!(checks, json!(ids));
    let expected: Vec<Value> = registries
        .pairs()
        .into_iter()
        .map(|pair| {
            json!({"check": pair.check, "lang": pair.lang, "support": SupportDescription::from(&pair.support)})
        })
        .collect();
    assert_eq!(
        cells.as_array().unwrap().len(),
        ids.len() * registries.languages.ids().len()
    );
    assert_eq!(cells, Value::Array(expected));
    assert_golden("describe/matrix", &envelope);
}
