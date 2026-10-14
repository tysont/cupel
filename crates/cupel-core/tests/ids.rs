// ABOUTME: Tests for Cupel's identifiers: run IDs, symbol and site hashes, and finding IDs.
// ABOUTME: Finding IDs are checked against vectors computed with an independent BLAKE3 implementation.
use cupel_core::{CheckId, FindingId, RunId, SiteHash, SymbolHash};
use proptest::prelude::*;

fn symbol() -> SymbolHash {
    "sh_4b1d9e0c4b1d9e0c".parse().unwrap()
}

#[test]
fn finding_id_matches_known_vector_for_a_site() {
    // blake3(b"tests.mutation\0mutant.survived\0sh_4b1d9e0c4b1d9e0c\0a1b2c3d4e5f60718")
    // = 3e81391d…, computed with the Python `blake3` package.
    let site: SiteHash = "a1b2c3d4e5f60718".parse().unwrap();
    let id = FindingId::compute(
        &CheckId::from("tests.mutation"),
        "mutant.survived",
        &symbol(),
        Some(&site),
    );
    assert_eq!(id.as_str(), "fnd_3e8139");
}

#[test]
fn finding_id_matches_known_vector_for_a_whole_symbol() {
    // blake3(b"static.complexity\0complexity.high\0sh_4b1d9e0c4b1d9e0c\0") = 3370d1f5…
    let id = FindingId::compute(
        &CheckId::from("static.complexity"),
        "complexity.high",
        &symbol(),
        None,
    );
    assert_eq!(id.as_str(), "fnd_3370d1");
}

#[test]
fn subject_finding_id_matches_known_vector() {
    // blake3(b"cli\0cli.not-implemented\0\0check") = 88808d37…, computed with the Python
    // `blake3` package (ADR-016).
    let id = FindingId::for_subject(&CheckId::from("cli"), "cli.not-implemented", "check");
    assert_eq!(id.as_str(), "fnd_88808d");
}

#[test]
fn subject_finding_id_depends_on_check_kind_and_subject() {
    let base = FindingId::for_subject(&CheckId::from("cli"), "cli.usage", "--format");
    assert_eq!(base.as_str(), "fnd_e2de2f");
    for variant in [
        FindingId::for_subject(&CheckId::from("doctor"), "cli.usage", "--format"),
        FindingId::for_subject(&CheckId::from("cli"), "cli.not-implemented", "--format"),
        FindingId::for_subject(&CheckId::from("cli"), "cli.usage", "--cwd"),
    ] {
        assert_ne!(base, variant);
    }
}

#[test]
fn subject_finding_ids_never_collide_with_symbol_finding_ids() {
    // A symbol hash is never empty, so the empty symbol slot separates the two input spaces:
    // a subject that looks like a site cannot reproduce a symbol finding's ID.
    let site: SiteHash = "a1b2c3d4e5f60718".parse().unwrap();
    let by_symbol = FindingId::compute(&CheckId::from("c"), "k", &symbol(), Some(&site));
    let by_subject = FindingId::for_subject(
        &CheckId::from("c"),
        "k",
        "sh_4b1d9e0c4b1d9e0c\0a1b2c3d4e5f60718",
    );
    assert_ne!(by_symbol, by_subject);
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
    let site: SiteHash = "a1b2c3d4e5f60718".parse().unwrap();
    let other_site: SiteHash = "a1b2c3d4e5f60719".parse().unwrap();
    let other_symbol: SymbolHash = "sh_4b1d9e0c4b1d9e0d".parse().unwrap();
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
fn symbol_hash_requires_prefix_and_sixteen_lowercase_hex_digits() {
    assert_eq!(symbol().as_str(), "sh_4b1d9e0c4b1d9e0c");
    for bad in [
        "4b1d9e0c4b1d9e0c",
        "sh_",
        "sh_4b1d9e0c",
        "sh_4b1d9e0c4b1d9e0c0",
        "sh_XYZd9e0c4b1d9e0c",
        "sh_4B1D9E0C4B1D9E0C",
        "fnd_4b1d9e0c4b1d9e0c",
    ] {
        assert!(
            bad.parse::<SymbolHash>().is_err(),
            "{bad} should be rejected"
        );
        assert!(serde_json::from_str::<SymbolHash>(&format!("\"{bad}\"")).is_err());
    }
}

#[test]
fn site_hash_requires_sixteen_lowercase_hex_digits() {
    for bad in [
        "",
        "a1",
        "A1B2C3D4E5F60718",
        "sh_a1b2c3d4e5f60718",
        "a1b2c3d4e5f607180",
    ] {
        assert!(bad.parse::<SiteHash>().is_err(), "{bad} should be rejected");
    }
    assert_eq!(
        "a1b2c3d4e5f60718".parse::<SiteHash>().unwrap().as_str(),
        "a1b2c3d4e5f60718"
    );
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
        symbol_hex in "[0-9a-f]{16}",
        site_hex in proptest::option::of("[0-9a-f]{16}"),
    ) {
        let symbol: SymbolHash = format!("sh_{symbol_hex}").parse().unwrap();
        let site = site_hex.map(|h| h.parse::<SiteHash>().unwrap());
        let first = FindingId::compute(&CheckId::from(check.as_str()), &kind, &symbol, site.as_ref());
        let again = FindingId::compute(&CheckId::from(check.as_str()), &kind, &symbol, site.as_ref());
        prop_assert_eq!(&first, &again);
        prop_assert!(first.as_str().parse::<FindingId>().is_ok());
    }
}
