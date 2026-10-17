// ABOUTME: Tests for the language registry: the compiled-in languages, their families and extensions,
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
fn the_compiled_in_languages_are_registered_with_their_families() {
    let registry = LanguageRegistry::builtin();
    let languages: Vec<(String, String, Stability)> = registry
        .iter()
        .map(|language| {
            (
                language.id().to_string(),
                language.family().to_string(),
                language.stability(),
            )
        })
        .collect();
    assert_eq!(
        languages,
        [
            ("go", "go"),
            ("java", "java"),
            ("javascript", "ecma"),
            ("python", "python"),
            ("rust", "rust"),
            ("typescript", "ecma"),
        ]
        .map(|(id, family)| (id.to_owned(), family.to_owned(), Stability::Experimental))
    );
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
        ("src/main/java/a/A.java", "java"),
    ];
    for (path, id) in cases {
        let language = registry.for_path(path).unwrap_or_else(|| panic!("{path}"));
        assert_eq!(language.id().as_str(), id, "{path}");
    }
    for path in ["README.md", "Makefile", "a.rs.orig", "noext"] {
        assert!(registry.for_path(path).is_none(), "{path}");
    }
    let mut extensions: Vec<&str> = registry
        .iter()
        .flat_map(|language| language.extensions().iter().copied())
        .collect();
    extensions.sort_unstable();
    assert_eq!(
        extensions,
        [
            ".cjs", ".cts", ".go", ".java", ".js", ".jsx", ".mjs", ".mts", ".py", ".rs", ".ts",
            ".tsx"
        ],
        "each extension is claimed once"
    );
}

/// Sources and whether they parse cleanly at a path: `.jsx` and `.js` take the JavaScript
/// grammar, which understands JSX; `.tsx` takes TSX; `.ts` and `.mts` take TypeScript, where an
/// angle-bracket type assertion is valid and in TSX it is not.
const PARSES: &[(&str, &str, bool)] = &[
    (
        "App.jsx",
        "const App = () => <div className=\"a\">{x}</div>;\n",
        true,
    ),
    (
        "App.js",
        "const App = () => <div className=\"a\">{x}</div>;\n",
        true,
    ),
    ("App.tsx", "const App = (p: P) => <div>{p.x}</div>;\n", true),
    ("a.ts", "const n = <number>value;\n", true),
    ("a.tsx", "const n = <number>value;\n", false),
    (
        "a.mts",
        "export const f = (x: number): number => x;\n",
        true,
    ),
];

#[test]
fn each_file_parses_with_the_grammar_its_extension_selects() {
    for (path, source, clean) in PARSES {
        assert_eq!(parses_cleanly(path, source), *clean, "{path}");
    }
}

#[test]
fn registering_producers_never_registers_a_language_twice_for_a_fact() {
    let registry = LanguageRegistry::builtin();
    let mut producers = ProducerRegistry::default();
    registry.register_producers(&mut producers);
    let langs = registry.langs();
    let twice: Vec<String> = langs
        .iter()
        .flat_map(|lang| {
            cupel_core::FactKind::ALL
                .into_iter()
                .map(move |kind| (lang, kind))
        })
        .filter(|(lang, kind)| producers.producers_for(*kind, lang).count() > 1)
        .map(|(lang, kind)| format!("{kind} for {}", lang.id))
        .collect();
    assert_eq!(twice, Vec::<String>::new());
}
