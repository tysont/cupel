// ABOUTME: Per-fact conformance (design §8.3.7): every registered language that produces a fact maps
// ABOUTME: every construct of that fact's inventory, and its producer detects each mapped construct.
use cupel_core::{FactEntry, FactKind, LangId, ProducerRegistry};
use cupel_fixtures::conformance::{
    FactInventory, Failure, LanguageInventory, check_language, execution, fact_inventories,
    fact_inventory, run_language, unexhibited,
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

#[test]
fn every_fact_inventory_loads_and_names_a_kind_whose_entry_has_conformance() {
    for inventory in fact_inventories() {
        assert!(inventory.fact.entry().conformance, "{}", inventory.fact);
        assert!(!inventory.constructs.is_empty(), "{}", inventory.fact);
    }
}

#[test]
fn every_kind_with_conformance_has_exactly_its_inventory() {
    let with_conformance: Vec<FactKind> = FactEntry::catalog()
        .iter()
        .filter(|entry| entry.conformance)
        .map(|entry| entry.kind)
        .collect();
    let mut inventories: Vec<FactKind> = fact_inventories()
        .iter()
        .map(|inventory| inventory.fact)
        .collect();
    inventories.sort();
    assert_eq!(inventories, with_conformance);
    for kind in execution() {
        assert_eq!(fact_inventory(kind).fact, kind);
    }
}

#[test]
fn every_syntactic_fact_is_produced_by_some_language() {
    let languages = LanguageRegistry::builtin();
    let mut producers = ProducerRegistry::default();
    languages.register_producers(&mut producers);
    let unproduced: Vec<FactKind> = FactKind::syntactic()
        .iter()
        .copied()
        .filter(|kind| {
            languages
                .langs()
                .iter()
                .all(|lang| producers.producers_for(*kind, lang).next().is_none())
        })
        .collect();
    assert_eq!(unproduced, Vec::<FactKind>::new());
}

#[test]
fn an_execution_fact_exhibits_each_construct_or_gives_a_reason() {
    let facts = inventory(
        "traces",
        r#"
        [[constructs]]
        id = "conforms"
        summary = "A trace the spec allows."
        within = "traces"
        expect = { status = "conforms" }

        [[constructs]]
        id = "diverged"
        summary = "A trace the spec does not allow."
        within = "traces"
        expect = { status = "diverged", divergence = { step = 1 } }

        [[constructs]]
        id = "failed"
        summary = "A trace that cannot be validated."
        within = "traces"
        expect = { status = "failed" }
        "#,
    );
    let fact = json!({ "traces": [
        { "name": "a.jsonl", "status": "conforms" },
        { "name": "b.jsonl", "status": "diverged", "divergence": { "step": 2 } },
    ] });

    let failures = unexhibited(&facts, &fact, &[("failed", "The tool reads every trace.")]);

    assert!(
        matches!(failures.as_slice(), [Failure::NotDetected { construct, .. }] if construct == "diverged"),
        "{failures:?}"
    );
    let failures = unexhibited(
        &facts,
        &fact,
        &[("failed", " "), ("misspelled", "A reason.")],
    );
    let constructs: Vec<&str> = failures
        .iter()
        .filter_map(|failure| match failure {
            Failure::NotDetected { construct, .. } => Some(construct.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(constructs, ["diverged", "failed"], "{failures:?}");
    assert!(
        matches!(&failures[0], Failure::Unknown { what } if what == "construct `traces.misspelled`"),
        "{failures:?}"
    );
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

#[test]
fn an_execution_fact_s_entry_is_located_by_the_file_its_producer_read() {
    let facts = [inventory(
        "histories",
        r#"
        [[constructs]]
        id = "stale-read"
        summary = "A stale read."
        within = "violations"
        expect = { anomaly = "non-linearizable" }
        "#,
    )];
    let lang = LanguageInventory::parse(
        &LangId::from("go"),
        "[histories.stale-read]\nfile = \"histories/stale-read.jsonl\"\n",
    )
    .unwrap();
    let checked = |violations: serde_json::Value| {
        let fact = json!({ "histories": [
            { "name": "histories/other.jsonl", "violations": [{ "anomaly": "non-linearizable" }] },
            { "name": "histories/stale-read.jsonl", "violations": violations },
        ] });
        check_language(&facts, &lang, &[FactKind::Histories], |_| Ok(fact.clone()))
    };

    assert_eq!(checked(json!([{ "anomaly": "non-linearizable" }])), []);
    assert!(
        matches!(checked(json!([])).as_slice(), [Failure::NotDetected { construct, .. }] if construct == "stale-read")
    );
}

#[test]
fn a_simulation_harness_is_located_by_its_file_and_name() {
    let facts = [inventory(
        "simulation-results",
        "[[constructs]]\nid = \"fails-at-a-seed\"\nsummary = \"A failing seed.\"\nexpect = { status = \"failed\" }\n",
    )];
    let lang = LanguageInventory::parse(
        &LangId::from("rust"),
        "[simulation-results.fails-at-a-seed]\nfile = \"tests/sim.rs\"\nname = \"lost\"\n",
    )
    .unwrap();
    let harness = |file: &str, name: &str, status: &str| json!({ "name": name, "location": { "file": file, "line": 3 }, "status": status });
    let checked = |harnesses: serde_json::Value| {
        let fact = json!({ "harnesses": harnesses });
        check_language(&facts, &lang, &[FactKind::SimulationResults], |_| {
            Ok(fact.clone())
        })
    };

    assert_eq!(
        checked(json!([harness("tests/sim.rs", "lost", "failed")])),
        []
    );
    assert_eq!(
        checked(json!([
            harness("tests/other.rs", "lost", "failed"),
            harness("tests/sim.rs", "lost", "passed")
        ]))
        .len(),
        1
    );
}

#[test]
fn a_reference_is_located_by_its_file_and_name_and_a_dynamic_access_by_its_text() {
    let facts = [inventory(
        "references",
        "[[constructs]]\nid = \"local\"\nsummary = \"A local name.\"\nexpect = { kind = \"name\" }\n\n[[constructs]]\nid = \"dynamic-name\"\nsummary = \"A lookup by name.\"\nexpect = { reach = \"names\" }\n",
    )];
    let lang = LanguageInventory::parse(
        &LangId::from("python"),
        "[references.local]\nfile = \"a.py\"\nname = \"helper\"\n\n[references.dynamic-name]\nfile = \"a.py\"\nname = \"getattr(m, kind)\"\n",
    )
    .unwrap();
    let checked = |references: serde_json::Value, dynamic: serde_json::Value| {
        let fact = json!({ "references": references, "dynamic": dynamic });
        check_language(&facts, &lang, &[FactKind::References], |_| Ok(fact.clone()))
    };
    let access = |file: &str| json!({ "file": file, "text": "getattr(m, kind)", "reach": "names" });

    assert_eq!(
        checked(
            json!([{ "file": "a.py", "name": "helper", "kind": "name" }]),
            json!([access("a.py")])
        ),
        []
    );
    assert_eq!(
        checked(
            json!([{ "file": "b.py", "name": "helper", "kind": "name" }]),
            json!([access("b.py")])
        )
        .len(),
        2
    );
}
