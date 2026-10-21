// ABOUTME: The References producers: plain names resolved in their file or through imports, members, strings,
// ABOUTME: attributes, implicit constructors, dynamic access, and coverage of every file of the language (ADR-135).
mod support;

use std::path::Path;

use cupel_core::syntax::{DynamicReach, ReferenceKind, References};
use support::{fixture, produce, produce_scoped, tree};

/// (kind, target kind, target lines) of each reference named `name` in `file`.
fn named(fact: &References, file: &str, name: &str) -> Vec<(ReferenceKind, String, Vec<u32>)> {
    fact.references
        .iter()
        .filter(|reference| reference.file == file && reference.name == name)
        .map(|reference| {
            let target = serde_json::to_value(&reference.target).unwrap();
            (
                reference.kind,
                target["kind"].as_str().unwrap().to_owned(),
                reference
                    .target
                    .symbols()
                    .iter()
                    .map(|symbol| symbol.line)
                    .collect(),
            )
        })
        .collect()
}

fn of(lang: &str, root: &Path) -> References {
    produce(lang, root)
}

#[test]
fn a_plain_name_resolves_to_the_function_of_that_name_in_its_own_file() {
    let fact = of("rust", &fixture("rust"));
    let found = named(&fact, "src/references.rs", "tiered_rate");
    assert_eq!(found, [(ReferenceKind::Name, "local".to_owned(), vec![17])]);
}

#[test]
fn a_plain_name_an_import_binds_resolves_to_the_function_in_the_imported_file() {
    let rust = of("rust", &fixture("rust"));
    let python = of("python", &fixture("python"));
    for (fact, file, name, line) in [
        (&rust, "src/rates.rs", "imported_rate", 13),
        (&python, "src/ledger/rates.py", "_imported_rate", 11),
    ] {
        let found = named(fact, file, name);
        assert!(!found.is_empty(), "{file} names {name}");
        for (kind, target, lines) in found {
            assert_eq!(
                (kind, target.as_str(), lines),
                (ReferenceKind::Name, "imported", vec![line])
            );
        }
    }
}

#[test]
fn an_es_import_binds_no_name_by_its_specifier_so_the_name_stays_unresolved() {
    let fact = of("javascript", &fixture("javascript"));
    for (_, target, _) in named(&fact, "src/ledger.js", "square") {
        assert_eq!(target, "unresolved");
    }
    let found = named(&fact, "src/rates.js", "importedRate");
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found.iter().all(|(_, target, _)| target == "unresolved"));
}

#[test]
fn a_name_reached_through_a_value_type_or_package_is_an_unresolved_member() {
    let cases = [
        ("rust", "src/references.rs", "iter"),
        ("go", "cmd/ledger/main.go", "NewAccount"),
        ("java", "src/main/java/ledger/Rates.java", "importedRate"),
        ("clojure", "src/ledger/rates.clj", "imported-rate"),
        ("python", "src/ledger/references.py", "modules"),
        ("typescript", "src/ledger.ts", "parse"),
    ];
    for (lang, file, name) in cases {
        let fact = of(lang, &fixture(lang));
        let found = named(&fact, file, name);
        assert!(
            !found.is_empty()
                && found
                    .iter()
                    .all(|(kind, target, _)| *kind == ReferenceKind::Member
                        && target == "unresolved"),
            "{lang} {file} {name}: {found:?}"
        );
    }
}

#[test]
fn a_definition_s_own_name_is_not_a_reference() {
    for (lang, file, name) in [
        ("rust", "src/references.rs", "orphan_rate"),
        ("python", "src/ledger/references.py", "_orphan_rate"),
        ("go", "references.go", "orphanRate"),
        ("javascript", "src/references.js", "orphanRate"),
        ("java", "src/main/java/ledger/References.java", "orphanRate"),
        ("clojure", "src/ledger/references.clj", "orphan-rate"),
    ] {
        let fact = of(lang, &fixture(lang));
        assert_eq!(named(&fact, file, name), [], "{lang}");
    }
}

#[test]
fn an_attribute_names_the_definition_it_is_on_unless_the_language_calls_it_inert() {
    let dir = tree(&[(
        "src/lib.rs",
        "#[inline]\nfn quick() {}\n\n#[test]\n#[should_panic]\nfn checked() {}\n\n#[serde(default = \"crate::config::port\")]\nstruct Config;\n",
    )]);
    let fact = of("rust", dir.path());
    let attributes: Vec<(String, Vec<u32>)> = fact
        .references
        .iter()
        .filter(|reference| reference.kind == ReferenceKind::Attribute)
        .map(|reference| {
            (
                reference.name.clone(),
                reference.target.symbols().iter().map(|s| s.line).collect(),
            )
        })
        .collect();
    assert_eq!(
        attributes,
        [
            ("test".to_owned(), vec![6]),
            ("should_panic".to_owned(), vec![6])
        ],
        "`inline` is inert, and an attribute on a struct names no symbol"
    );
}

#[test]
fn a_decorator_names_the_function_or_method_it_decorates() {
    let python = tree(&[(
        "app.py",
        "@app.route(\"/\")\ndef _index():\n    pass\n\n@staticmethod\ndef _plain():\n    pass\n",
    )]);
    let fact = of("python", python.path());
    assert_eq!(
        named(&fact, "app.py", "route"),
        [
            (ReferenceKind::Attribute, "local".to_owned(), vec![2]),
            (ReferenceKind::Member, "unresolved".to_owned(), vec![])
        ],
        "the decorator names `_index`, and `app.route` is a member"
    );
    assert_eq!(
        named(&fact, "app.py", "staticmethod"),
        [(ReferenceKind::Name, "unresolved".to_owned(), vec![])],
        "an inert decorator is only the name it is written with"
    );
    let java = fixture("java");
    let fact = of("java", &java);
    let found = named(&fact, "src/test/java/ledger/LedgerTest.java", "BeforeEach");
    assert!(
        found.contains(&(ReferenceKind::Attribute, "local".to_owned(), vec![16])),
        "{found:?}"
    );
}

#[test]
fn a_string_whose_text_is_a_path_of_names_is_a_text_reference_to_its_last_segment() {
    let dir = tree(&[(
        "src/lib.rs",
        "#[serde(default = \"crate::config::port\")]\nstruct Config;\nconst NOTE: &str = \"Return the total.\";\n",
    )]);
    let fact = of("rust", dir.path());
    let texts: Vec<&str> = fact
        .references
        .iter()
        .filter(|reference| reference.kind == ReferenceKind::Text)
        .map(|reference| reference.name.as_str())
        .collect();
    assert_eq!(texts, ["port"]);
}

#[test]
fn identifiers_in_a_macro_are_members_since_the_token_tree_does_not_say_what_they_are() {
    let dir = tree(&[(
        "src/lib.rs",
        "fn len() -> usize { 0 }\nfn check(v: &[u8]) { assert!(v.len() == len()); }\n",
    )]);
    let fact = of("rust", dir.path());
    let found = named(&fact, "src/lib.rs", "len");
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found
            .iter()
            .all(|(kind, target, _)| *kind == ReferenceKind::Member && target == "unresolved"),
        "{found:?}"
    );
}

#[test]
fn a_constructor_the_runtime_calls_gets_an_implicit_reference_to_itself() {
    let fact = of("javascript", &fixture("javascript"));
    assert_eq!(
        named(&fact, "src/complexity.js", "constructor"),
        [(ReferenceKind::Implicit, "local".to_owned(), vec![129])]
    );
}

#[test]
fn a_name_computed_at_run_time_is_a_dynamic_access_with_what_it_can_reach() {
    let python = tree(&[(
        "m.py",
        "import sys\n\ndef run(kind):\n    getattr(sys.modules[__name__], kind)()\n    getattr(sys, \"exit\")\n    return eval(kind)\n",
    )]);
    let fact = of("python", python.path());
    let python_sites: Vec<(u32, DynamicReach)> = fact
        .dynamic
        .iter()
        .map(|access| (access.span.line, access.reach))
        .collect();
    assert_eq!(
        python_sites,
        [(4, DynamicReach::Names), (6, DynamicReach::Names)],
        "`getattr` with a literal name is a text reference, not a dynamic access"
    );
    let fact = of("typescript", &fixture("typescript"));
    let sites: Vec<(&str, DynamicReach)> = fact
        .dynamic
        .iter()
        .filter(|access| access.file == "src/references.ts")
        .map(|access| (access.text.as_str(), access.reach))
        .collect();
    assert_eq!(
        sites,
        [
            ("RATES[kind]", DynamicReach::Members),
            ("eval(source)", DynamicReach::Names)
        ]
    );
    for lang in ["rust", "go"] {
        assert_eq!(of(lang, &fixture(lang)).dynamic, [], "{lang}");
    }
}

#[test]
fn references_come_from_every_file_of_the_language_whatever_the_scope() {
    let fact: References = produce_scoped("rust", &fixture("rust"), &["src/references.rs"]);
    let found = named(&fact, "src/rates.rs", "imported_rate");
    assert!(
        found.iter().any(|(_, target, _)| target == "imported"),
        "a caller outside the scope is read: {found:?}"
    );
}
