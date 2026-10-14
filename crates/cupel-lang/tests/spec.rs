// ABOUTME: Tests for language specs: every embedded query compiles against every grammar of its
// ABOUTME: language, uses only contract capture names, and every `.scm` on disk is embedded.
use std::fs;
use std::path::{Path, PathBuf};

use cupel_lang::{LanguageRegistry, QueryFile, contract};

/// The grammars each language parses with, by a representative path for each.
const GRAMMAR_PATHS: [(&str, &[&str]); 5] = [
    ("javascript", &["a.js"]),
    ("typescript", &["a.ts", "a.tsx"]),
    ("rust", &["a.rs"]),
    ("go", &["a.go"]),
    ("python", &["a.py"]),
];

/// Query directories on disk and the languages whose specs embed them.
const QUERY_DIRS: [(&str, &[&str]); 6] = [
    ("ecma/queries", &["javascript", "typescript"]),
    ("ecma/queries/javascript", &["javascript"]),
    ("ecma/queries/typescript", &["typescript"]),
    ("rust/queries", &["rust"]),
    ("go/queries", &["go"]),
    ("python/queries", &["python"]),
];

fn langs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/langs")
}

#[test]
fn every_query_compiles_against_every_grammar_of_its_language_with_contract_captures() {
    let registry = LanguageRegistry::builtin();
    let problems: Vec<String> = GRAMMAR_PATHS
        .iter()
        .flat_map(|(id, paths)| {
            let language = registry.get(&(*id).into()).unwrap();
            QueryFile::ALL
                .into_iter()
                .filter(|file| language.spec().source(*file).is_some())
                .flat_map(move |file| paths.iter().map(move |path| (id, file, path)))
                .filter_map(|(id, file, path)| {
                    let grammar = language.grammar(Path::new(path));
                    language
                        .spec()
                        .query(file, &grammar)
                        .map_err(|error| format!("{id} {file} ({path}): {error}"))
                        .and_then(|query| {
                            contract(file)
                                .check(query.capture_names())
                                .map_err(|problems| format!("{id} {file}: {}", problems.join("; ")))
                        })
                        .err()
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(problems, Vec::<String>::new());
}

#[test]
fn every_scm_file_on_disk_is_embedded_in_the_specs_that_use_its_directory() {
    let registry = &LanguageRegistry::builtin();
    let missing: Vec<String> = QUERY_DIRS
        .iter()
        .flat_map(|(dir, ids)| {
            query_files(dir).into_iter().flat_map(move |path| {
                let name = path.file_name().unwrap().to_str().unwrap().to_owned();
                let file = QueryFile::from_file_name(&name)
                    .unwrap_or_else(|| panic!("{dir}/{name} is not a known query file"));
                let text = fs::read_to_string(&path).unwrap();
                ids.iter()
                    .filter(move |id| {
                        let spec = registry.get(&(**id).into()).unwrap().spec();
                        !spec.source(file).unwrap_or_default().contains(text.trim())
                    })
                    .map(move |id| format!("{dir}/{name} is not embedded in the {id} spec"))
            })
        })
        .collect();
    assert_eq!(missing, Vec::<String>::new());
}

/// The `.scm` files directly in the query directory `dir`; none when it does not exist.
fn query_files(dir: &str) -> Vec<PathBuf> {
    fs::read_dir(langs_dir().join(dir))
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "scm"))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn every_language_has_tables_that_parse() {
    let registry = LanguageRegistry::builtin();
    // Reading the tables forces the one-time parse; a malformed file panics here.
    let frameworks: Vec<&str> = registry
        .iter()
        .map(|language| language.spec().tables().tests.framework.as_str())
        .collect();
    assert_eq!(
        frameworks,
        ["go-test", "jest", "pytest", "rust-test", "jest"]
    );
}

#[test]
fn the_contract_rejects_unknown_and_missing_captures() {
    let problems = contract(QueryFile::Symbols)
        .check(&["symbol.function", "symbol.nmae", "_ignored"])
        .unwrap_err();
    assert_eq!(
        problems,
        [
            "@symbol.nmae is not a capture symbols.scm may use",
            "symbols.scm must capture @symbol.name",
            "symbols.scm must capture @symbol.parameter",
        ]
    );
    assert_eq!(
        contract(QueryFile::Symbols).check(&[
            "symbol.method",
            "symbol.name",
            "symbol.parameter",
            "_t"
        ]),
        Ok(())
    );
}
