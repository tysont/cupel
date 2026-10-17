// ABOUTME: Per-fact conformance (design §8.3.7): every registered language that produces a fact maps
// ABOUTME: every construct of that fact's inventory, and its producer detects each mapped construct.
use cupel_core::{FactKind, LangId};
use cupel_fixtures::conformance::{
    FactInventory, Failure, LanguageInventory, check_language, fact_inventories, run_language,
};
use cupel_lang::LanguageRegistry;
use serde_json::json;

/// The languages to check: all registered ones, or only `CONFORMANCE_LANG` when it is set.
fn languages() -> Vec<LangId> {
    let all = LanguageRegistry::builtin().ids();
    match std::env::var("CONFORMANCE_LANG") {
        Ok(only) => all.into_iter().filter(|id| id.as_str() == only).collect(),
        Err(_) => all,
    }
}

#[test]
fn every_language_conforms_for_every_fact_it_produces() {
    let mut report = Vec::new();
    for lang in languages() {
        for failure in run_language(&lang) {
            report.push(format!("{lang}: {failure}"));
        }
    }
    assert!(report.is_empty(), "\n{}", report.join("\n"));
}

#[test]
fn every_registered_language_has_a_fixture_repository_and_inventory() {
    for lang in LanguageRegistry::builtin().ids() {
        let inventory = cupel_fixtures::repo_path(lang.as_str()).join("inventory.toml");
        assert!(inventory.is_file(), "{} is missing", inventory.display());
        LanguageInventory::load(&lang).unwrap();
    }
}

/// Execution facts whose inventories their producers' capture tests check, construct by
/// construct, on real tool output (ADR-113), rather than languages' fixture repositories.
const CHECKED_ON_CAPTURES: [FactKind; 1] = [FactKind::ContractViolations];

#[test]
fn every_fact_inventory_loads_and_names_a_syntactic_fact_or_one_checked_on_captures() {
    for inventory in fact_inventories() {
        assert!(
            FactKind::SYNTACTIC.contains(&inventory.fact)
                || CHECKED_ON_CAPTURES.contains(&inventory.fact),
            "{}",
            inventory.fact
        );
        assert!(!inventory.constructs.is_empty(), "{}", inventory.fact);
    }
}

fn inventory(fact: &str, constructs: &str) -> FactInventory {
    FactInventory::parse(&format!("fact = \"{fact}\"\n{constructs}")).unwrap()
}

const TWO_CONSTRUCTS: &str = r#"
[[constructs]]
id = "method"
summary = "A method."
expect = { kind = "method" }

[[constructs]]
id = "closure"
summary = "A closure bound to a name."
expect = { kind = "closure" }
"#;

fn symbols_fact() -> serde_json::Value {
    json!({ "symbols": [
        { "display": "Point::norm", "file": "src/a.rs", "kind": "method", "line": 1 },
    ] })
}

#[test]
fn the_harness_fails_on_a_construct_the_language_does_not_map() {
    let facts = [inventory("symbols", TWO_CONSTRUCTS)];
    let lang = LanguageInventory::parse(
        &LangId::from("rust"),
        r#"
        [symbols.method]
        file = "src/a.rs"
        name = "Point::norm"
        "#,
    )
    .unwrap();

    let failures = check_language(&facts, &lang, &[FactKind::Symbols], |_| Ok(symbols_fact()));

    assert_eq!(
        failures,
        [Failure::Unmapped {
            fact: FactKind::Symbols,
            construct: "closure".to_owned()
        }]
    );
}

#[test]
fn not_applicable_needs_a_reason_and_counts_as_mapped() {
    let facts = [inventory("symbols", TWO_CONSTRUCTS)];
    let lang = LanguageInventory::parse(
        &LangId::from("rust"),
        r#"
        [symbols.method]
        file = "src/a.rs"
        name = "Point::norm"

        [symbols.closure]
        not-applicable = "This language has no closures."
        "#,
    )
    .unwrap();
    assert!(check_language(&facts, &lang, &[FactKind::Symbols], |_| Ok(symbols_fact())).is_empty());

    let empty_reason = LanguageInventory::parse(
        &LangId::from("rust"),
        "[symbols.closure]\nnot-applicable = \"\"\n",
    );
    assert!(empty_reason.is_err());
}

#[test]
fn the_harness_fails_on_a_mapped_construct_the_producer_does_not_detect() {
    let facts = [inventory("symbols", TWO_CONSTRUCTS)];
    let lang = LanguageInventory::parse(
        &LangId::from("rust"),
        r#"
        [symbols.method]
        file = "src/a.rs"
        name = "Point::norm"

        [symbols.closure]
        file = "src/a.rs"
        name = "Point::norm"
        "#,
    )
    .unwrap();

    let failures = check_language(&facts, &lang, &[FactKind::Symbols], |_| Ok(symbols_fact()));

    assert!(
        matches!(failures.as_slice(), [Failure::NotDetected { construct, .. }] if construct == "closure"),
        "{failures:?}"
    );
}

#[test]
fn mappings_to_unknown_constructs_or_facts_are_failures() {
    let facts = [inventory("symbols", TWO_CONSTRUCTS)];
    let lang = LanguageInventory::parse(
        &LangId::from("rust"),
        r#"
        [symbols.method]
        file = "src/a.rs"
        name = "Point::norm"

        [symbols.closure]
        not-applicable = "No closures."

        [symbols.lambda]
        not-applicable = "Misspelled construct."

        [symbolz.method]
        not-applicable = "Misspelled fact."
        "#,
    )
    .unwrap();

    let failures = check_language(&facts, &lang, &[FactKind::Symbols], |_| Ok(symbols_fact()));

    assert_eq!(failures.len(), 2, "{failures:?}");
    assert!(
        failures
            .iter()
            .all(|f| matches!(f, Failure::Unknown { .. }))
    );
}

#[test]
fn facts_without_a_producer_are_not_checked() {
    let facts = [inventory("decisions", TWO_CONSTRUCTS)];
    let lang = LanguageInventory::parse(&LangId::from("rust"), "").unwrap();
    assert!(check_language(&facts, &lang, &[FactKind::Symbols], |_| unreachable!()).is_empty());
}
