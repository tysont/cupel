// ABOUTME: The fact catalog derived from each payload module's submission (ADR-123): one entry per `FactKind`
// ABOUTME: variant in declaration order, named as it serializes, each with a versioned schema and a decoder.
use std::collections::BTreeSet;

use cupel_core::{FactEntry, FactKind, schema};
use serde_json::{Value, json};

/// Every `FactKind` variant's name in declaration order, read from the enum's derived schema.
fn declared() -> Vec<String> {
    let schema = schemars::schema_for!(FactKind);
    serde_json::from_value(schema.as_value()["enum"].clone()).unwrap()
}

fn names(entries: &[&FactEntry]) -> Vec<String> {
    entries.iter().map(|entry| entry.name.to_owned()).collect()
}

#[test]
fn every_variant_has_exactly_one_entry_in_declaration_order() {
    assert_eq!(names(FactEntry::catalog()), declared());
    let kinds: Vec<String> = FactKind::all().iter().map(ToString::to_string).collect();
    assert_eq!(kinds, declared());
}

#[test]
fn every_entry_is_named_in_kebab_case_as_its_kind_serializes_and_displays() {
    let kebab = |name: &str| {
        !name.is_empty()
            && name
                .split('-')
                .all(|word| !word.is_empty() && word.chars().all(|c| c.is_ascii_lowercase()))
    };
    let misnamed: Vec<&str> = FactEntry::catalog()
        .iter()
        .filter(|entry| {
            !kebab(entry.name)
                || serde_json::to_value(entry.kind).unwrap() != json!(entry.name)
                || entry.kind.to_string() != entry.name
                || entry.kind.as_str() != entry.name
        })
        .map(|entry| entry.name)
        .collect();
    assert_eq!(misnamed, Vec::<&str>::new());
    let unique: BTreeSet<&str> = FactEntry::catalog()
        .iter()
        .map(|entry| entry.name)
        .collect();
    assert_eq!(unique.len(), FactEntry::catalog().len());
}

/// What `schema::fact` serves for an entry's kind: its `$id`, its version, and `fact_version`.
fn served(entry: &FactEntry) -> (Value, Value, Option<u32>) {
    let schema = schema::fact(entry.kind).unwrap();
    (
        schema.as_value()["$id"].clone(),
        schema.as_value()["x-cupel-fact-version"].clone(),
        schema::fact_version(entry.kind),
    )
}

#[test]
fn every_entry_serves_a_schema_at_its_version_of_at_least_one() {
    let unversioned: Vec<&str> = FactEntry::catalog()
        .iter()
        .filter(|entry| entry.version < 1)
        .map(|entry| entry.name)
        .collect();
    assert_eq!(unversioned, Vec::<&str>::new());
    let served: Vec<(Value, Value, Option<u32>)> = FactEntry::catalog()
        .iter()
        .map(|entry| served(entry))
        .collect();
    let expected: Vec<(Value, Value, Option<u32>)> = FactEntry::catalog()
        .iter()
        .map(|entry| {
            let id = format!("https://cupel.dev/schema/v1/fact/{}.json", entry.name);
            (json!(id), json!(entry.version), Some(entry.version))
        })
        .collect();
    assert_eq!(served, expected);
}

#[test]
fn the_syntactic_kinds_are_the_entries_marked_syntactic_and_each_has_conformance() {
    let marked: Vec<FactKind> = FactEntry::catalog()
        .iter()
        .filter(|entry| entry.syntactic)
        .map(|entry| entry.kind)
        .collect();
    assert_eq!(FactKind::syntactic(), marked.as_slice());
    let without: Vec<&str> = FactEntry::catalog()
        .iter()
        .filter(|entry| entry.syntactic && !entry.conformance)
        .map(|entry| entry.name)
        .collect();
    assert_eq!(without, Vec::<&str>::new());
}

#[test]
fn every_entry_decodes_only_its_own_payload_shape() {
    let accepting: Vec<&str> = FactEntry::catalog()
        .iter()
        .filter(|entry| (entry.decode)(json!("not a payload")).is_ok())
        .map(|entry| entry.name)
        .collect();
    assert_eq!(accepting, Vec::<&str>::new());
    let decoded = (FactKind::Symbols.entry().decode)(json!({"symbols": []})).unwrap();
    assert_eq!(decoded.kind(), FactKind::Symbols);
    assert_eq!(decoded.to_json(), json!({"symbols": []}));
}

/// The schemas come from the entries, so `schema.rs` holds no per-kind table to edit (ADR-123).
#[test]
fn the_schema_module_names_no_fact_kind() {
    let naming: Vec<&str> = include_str!("../src/schema.rs")
        .lines()
        .filter(|line| line.contains(concat!("FactKind", "::")))
        .collect();
    assert_eq!(naming, Vec::<&str>::new());
}

#[test]
fn pin_keys_are_unique_and_come_from_the_entries() {
    let keys: Vec<&str> = FactEntry::catalog()
        .iter()
        .filter_map(|entry| entry.pin_key)
        .collect();
    let unique: BTreeSet<&str> = keys.iter().copied().collect();
    assert_eq!(unique.len(), keys.len());
    let read: Vec<Option<&str>> = FactKind::all().iter().map(|kind| kind.pin_key()).collect();
    let submitted: Vec<Option<&str>> = FactEntry::catalog()
        .iter()
        .map(|entry| entry.pin_key)
        .collect();
    assert_eq!(read, submitted);
}
