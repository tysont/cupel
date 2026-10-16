// ABOUTME: Tests for oracle identity and state (ADR-090 §4–5): what the identity hash covers, how a
// ABOUTME: recorded verification compares with it, and how an oracle reads in the envelope (ADR-091 §6).
use cupel_core::config::{OracleConfig, OracleKind};
use cupel_core::{OracleReport, OracleState, oracle_identity};
use proptest::prelude::*;
use serde_json::json;

fn reference(at: &str) -> OracleConfig {
    OracleConfig {
        kind: OracleKind::Ref,
        reference: Some(at.to_owned()),
        cmd: None,
        spec: None,
        lang: None,
        inputs: Vec::new(),
    }
}

#[test]
fn the_identity_is_a_blake3_hex_of_the_definition_and_what_it_resolves_to() {
    let identity = oracle_identity(&reference("v1"), Some("abc123"));
    assert_eq!(identity.len(), 64);
    assert!(identity.chars().all(|c| c.is_ascii_hexdigit()));
    let expected = blake3::hash(
        json!({"version": 1, "definition": reference("v1"), "resolved": "abc123"})
            .to_string()
            .as_bytes(),
    );
    assert_eq!(identity, expected.to_hex().to_string());
}

#[test]
fn the_identity_changes_with_the_definition_and_with_what_it_resolves_to() {
    let base = oracle_identity(&reference("v1"), Some("abc123"));
    assert_eq!(oracle_identity(&reference("v1"), Some("abc123")), base);
    assert_ne!(oracle_identity(&reference("v2"), Some("abc123")), base);
    assert_ne!(oracle_identity(&reference("v1"), Some("def456")), base);
    assert_ne!(oracle_identity(&reference("v1"), None), base);
}

#[test]
fn an_oracle_is_verified_only_while_its_recorded_hash_is_its_identity_now() {
    let cases = [
        (Some("h1"), Some("h1"), OracleState::Verified),
        (Some("h1"), None, OracleState::Unverified),
        (Some("h1"), Some("h0"), OracleState::Changed),
        (None, Some("h1"), OracleState::Unreachable),
        (None, None, OracleState::Unreachable),
    ];
    let states: Vec<OracleState> = cases
        .iter()
        .map(|(now, recorded, _)| OracleState::of(*now, *recorded))
        .collect();
    let expected: Vec<OracleState> = cases.iter().map(|(_, _, state)| *state).collect();
    assert_eq!(states, expected);
}

#[test]
fn a_report_names_its_state_in_kebab_case_with_its_verification() {
    let report = OracleReport {
        name: "old".to_owned(),
        definition: reference("v1"),
        state: OracleState::Changed,
        hash: Some("h1".to_owned()),
        verified_at: Some("2026-10-16T09:30:00Z".to_owned()),
        verified_run: Some("run_01JA2B3C4D5E6F7G8H9JKMNPQA".parse().unwrap()),
    };
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        json!({
            "name": "old",
            "definition": {"kind": "ref", "ref": "v1", "cmd": null, "spec": null, "lang": null,
                           "inputs": []},
            "state": "changed",
            "hash": "h1",
            "verified_at": "2026-10-16T09:30:00Z",
            "verified_run": "run_01JA2B3C4D5E6F7G8H9JKMNPQA"
        })
    );
}

proptest! {
    #[test]
    fn only_a_matching_recorded_hash_verifies(now in "[0-9a-f]{4}", recorded in "[0-9a-f]{4}") {
        let state = OracleState::of(Some(&now), Some(&recorded));
        prop_assert_eq!(state == OracleState::Verified, now == recorded);
    }
}
