// ABOUTME: Tests for Cupel's identifiers: run IDs, symbol and site hashes, and finding IDs.
// ABOUTME: Finding IDs are checked against vectors computed with an independent BLAKE3 implementation.
use cupel_core::{CheckId, FindingId, RunId, SiteHash, SymbolHash};
use proptest::prelude::*;

fn symbol() -> SymbolHash {
    "sh_4b1d9e0c".parse().unwrap()
}

#[test]
fn finding_id_matches_known_vector_for_a_site() {
    // blake3(b"tests.mutation\0mutant.survived\0sh_4b1d9e0c\0a1b2c3") = 41ee1ce7…, computed with
    // the Python `blake3` package.
    let site: SiteHash = "a1b2c3".parse().unwrap();
    let id = FindingId::compute(
        &CheckId::from("tests.mutation"),
        "mutant.survived",
        &symbol(),
        Some(&site),
    );
    assert_eq!(id.as_str(), "fnd_41ee1c");
}

#[test]
fn finding_id_matches_known_vector_for_a_whole_symbol() {
    // blake3(b"static.complexity\0complexity.high\0sh_4b1d9e0c\0") = 5e337c51…
    let id = FindingId::compute(
        &CheckId::from("static.complexity"),
        "complexity.high",
        &symbol(),
        None,
    );
    assert_eq!(id.as_str(), "fnd_5e337c");
}

#[test]
fn finding_id_keeps_24_bits_as_six_hex_digits_after_prefix() {
    let id = FindingId::compute(&CheckId::from("a"), "b", &symbol(), None);
    let hex = id.as_str().strip_prefix("fnd_").expect("fnd_ prefix");
    assert_eq!(hex.len(), 6);
    assert!(
        hex.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
}

#[test]
fn finding_id_parts_are_separated_unambiguously() {
    let a = FindingId::compute(&CheckId::from("ab"), "c", &symbol(), None);
    let b = FindingId::compute(&CheckId::from("a"), "bc", &symbol(), None);
    assert_ne!(a, b);
}

#[test]
fn finding_id_depends_on_every_structural_input() {
    let site: SiteHash = "a1b2c3".parse().unwrap();
    let other_site: SiteHash = "a1b2c4".parse().unwrap();
    let other_symbol: SymbolHash = "sh_4b1d9e0d".parse().unwrap();
    let base = FindingId::compute(&CheckId::from("c"), "k", &symbol(), Some(&site));
    for variant in [
        FindingId::compute(&CheckId::from("c2"), "k", &symbol(), Some(&site)),
        FindingId::compute(&CheckId::from("c"), "k2", &symbol(), Some(&site)),
        FindingId::compute(&CheckId::from("c"), "k", &other_symbol, Some(&site)),
        FindingId::compute(&CheckId::from("c"), "k", &symbol(), Some(&other_site)),
        FindingId::compute(&CheckId::from("c"), "k", &symbol(), None),
    ] {
        assert_ne!(base, variant);
    }
}

#[test]
fn finding_id_parses_and_round_trips_through_json() {
    let id: FindingId = "fnd_7c2e1a".parse().unwrap();
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"fnd_7c2e1a\"");
    assert_eq!(serde_json::from_str::<FindingId>(&json).unwrap(), id);
}

#[test]
fn finding_id_rejects_malformed_strings() {
    for bad in [
        "7c2e1a",
        "fnd_7c2e1",
        "fnd_7c2e1a0",
        "fnd_7C2E1A",
        "fnd_7c2e1g",
        "fn_7c2e1a",
    ] {
        assert!(
            bad.parse::<FindingId>().is_err(),
            "{bad} should be rejected"
        );
        assert!(serde_json::from_str::<FindingId>(&format!("\"{bad}\"")).is_err());
    }
}

#[test]
fn symbol_hash_requires_prefix_and_lowercase_hex() {
    assert_eq!(symbol().as_str(), "sh_4b1d9e0c");
    for bad in ["4b1d", "sh_", "sh_XYZ", "sh_4B1D", "fnd_4b1d"] {
        assert!(
            bad.parse::<SymbolHash>().is_err(),
            "{bad} should be rejected"
        );
        assert!(serde_json::from_str::<SymbolHash>(&format!("\"{bad}\"")).is_err());
    }
}

#[test]
fn site_hash_requires_lowercase_hex() {
    assert!("".parse::<SiteHash>().is_err());
    assert!("A1".parse::<SiteHash>().is_err());
    assert!("sh_a1".parse::<SiteHash>().is_err());
    assert_eq!("a1".parse::<SiteHash>().unwrap().as_str(), "a1");
}

#[test]
fn run_ids_are_prefixed_ulids_and_unique() {
    let a = RunId::generate();
    let b = RunId::generate();
    assert_ne!(a, b);
    let text = a.to_string();
    assert!(text.starts_with("run_"), "{text}");
    assert_eq!(text.len(), 4 + 26);
    assert_eq!(text.parse::<RunId>().unwrap(), a);
}

#[test]
fn run_id_round_trips_through_json_and_rejects_bad_input() {
    let id: RunId = "run_01JA2B3C4D5E6F7G8H9JKMNPQR".parse().unwrap();
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"run_01JA2B3C4D5E6F7G8H9JKMNPQR\"");
    assert_eq!(serde_json::from_str::<RunId>(&json).unwrap(), id);
    for bad in [
        "01JA2B3C4D5E6F7G8H9JKMNPQR",
        "run_",
        "run_not-a-ulid",
        "fnd_01JA2B3C4D5E6F7G8H9JKMNPQR",
    ] {
        assert!(bad.parse::<RunId>().is_err(), "{bad} should be rejected");
    }
}

proptest! {
    /// Finding IDs are a pure function of the structural inputs the API accepts (check, kind,
    /// symbol hash, site hash); there is no parameter through which a line number could enter.
    #[test]
    fn finding_id_is_deterministic_and_well_formed(
        check in "[a-z]{1,8}\\.[a-z-]{1,12}",
        kind in "[a-z]{1,8}\\.[a-z-]{1,12}",
        symbol_hex in "[0-9a-f]{1,32}",
        site_hex in proptest::option::of("[0-9a-f]{1,16}"),
    ) {
        let symbol: SymbolHash = format!("sh_{symbol_hex}").parse().unwrap();
        let site = site_hex.map(|h| h.parse::<SiteHash>().unwrap());
        let first = FindingId::compute(&CheckId::from(check.as_str()), &kind, &symbol, site.as_ref());
        let again = FindingId::compute(&CheckId::from(check.as_str()), &kind, &symbol, site.as_ref());
        prop_assert_eq!(&first, &again);
        prop_assert!(first.as_str().parse::<FindingId>().is_ok());
    }
}
