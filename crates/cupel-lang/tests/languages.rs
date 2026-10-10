// ABOUTME: Tests for the language registry: the five launch languages, their families and extensions,
// ABOUTME: lookup by ID and by path, and which grammar each path is parsed with.
use std::path::Path;

use cupel_core::{LangFamily, LangId, ProducerRegistry, Stability};
use cupel_lang::LanguageRegistry;

fn parses_cleanly(path: &str, source: &str) -> bool {
    let registry = LanguageRegistry::builtin();
    let language = registry
        .for_path(path)
        .unwrap_or_else(|| panic!("{path} has a language"));
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language.grammar(Path::new(path)))
        .unwrap();
    !parser.parse(source, None).unwrap().root_node().has_error()
}

#[test]
fn the_five_launch_languages_are_registered_with_their_families() {
    let registry = LanguageRegistry::builtin();
    let languages: Vec<(String, String)> = registry
        .iter()
        .map(|language| (language.id().to_string(), language.family().to_string()))
        .collect();
    assert_eq!(
        languages,
        [
            ("go", "go"),
            ("javascript", "ecma"),
            ("python", "python"),
            ("rust", "rust"),
            ("typescript", "ecma"),
        ]
        .map(|(id, family)| (id.to_owned(), family.to_owned()))
    );
    for language in registry.iter() {
        assert_eq!(language.stability(), Stability::Experimental);
    }
}

#[test]
fn languages_are_found_by_id_and_unknown_ids_are_not() {
    let registry = LanguageRegistry::builtin();
    assert_eq!(
        registry.get(&LangId::from("typescript")).unwrap().family(),
        LangFamily::from("ecma")
    );
    assert!(registry.get(&LangId::from("cobol")).is_none());
}

#[test]
fn every_extension_maps_to_exactly_one_language() {
    let registry = LanguageRegistry::builtin();
    let cases = [
        ("a.js", "javascript"),
        ("a.jsx", "javascript"),
        ("a.mjs", "javascript"),
        ("a.cjs", "javascript"),
        ("a.ts", "typescript"),
        ("a.mts", "typescript"),
        ("a.cts", "typescript"),
        ("src/a.tsx", "typescript"),
        ("src/lib.rs", "rust"),
        ("cmd/main.go", "go"),
        ("pkg/mod.py", "python"),
    ];
    for (path, id) in cases {
        let language = registry.for_path(path).unwrap_or_else(|| panic!("{path}"));
        assert_eq!(language.id().as_str(), id, "{path}");
    }
    for path in ["README.md", "Makefile", "a.rs.orig", "noext"] {
        assert!(registry.for_path(path).is_none(), "{path}");
    }
    let mut seen = Vec::new();
    for language in registry.iter() {
        for extension in language.extensions() {
            assert!(extension.starts_with('.'), "{extension}");
            assert!(!seen.contains(extension), "{extension} is claimed twice");
            seen.push(*extension);
        }
    }
}

#[test]
fn jsx_and_tsx_parse_with_the_grammar_that_understands_them() {
    let jsx = "const App = () => <div className=\"a\">{x}</div>;\n";
    assert!(parses_cleanly("App.jsx", jsx));
    assert!(parses_cleanly("App.js", jsx));
    assert!(parses_cleanly(
        "App.tsx",
        "const App = (p: P) => <div>{p.x}</div>;\n"
    ));
}

#[test]
fn ts_files_use_the_typescript_grammar_not_tsx() {
    // An angle-bracket type assertion is valid TypeScript and invalid TSX.
    let assertion = "const n = <number>value;\n";
    assert!(parses_cleanly("a.ts", assertion));
    assert!(!parses_cleanly("a.tsx", assertion));
    assert!(parses_cleanly(
        "a.mts",
        "export const f = (x: number): number => x;\n"
    ));
}

#[test]
fn registering_producers_never_registers_a_language_twice_for_a_fact() {
    let registry = LanguageRegistry::builtin();
    let mut producers = ProducerRegistry::default();
    registry.register_producers(&mut producers);
    for lang in registry.langs() {
        for kind in cupel_core::FactKind::ALL {
            let count = producers.producers_for(kind, &lang).count();
            assert!(count <= 1, "{kind} for {}: {count} producers", lang.id);
        }
    }
}
