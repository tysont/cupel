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
    for (id, paths) in GRAMMAR_PATHS {
        let language = registry.get(&id.into()).unwrap();
        for file in QueryFile::ALL {
            if language.spec().source(file).is_none() {
                continue;
            }
            for path in paths {
                let grammar = language.grammar(Path::new(path));
                let query = language
                    .spec()
                    .query(file, &grammar)
                    .unwrap_or_else(|error| panic!("{id} {file} ({path}): {error}"));
                let names: Vec<&str> = query.capture_names().to_vec();
                if let Err(problems) = contract(file).check(&names) {
                    panic!("{id} {file}: {}", problems.join("; "));
                }
            }
        }
    }
}

#[test]
fn every_scm_file_on_disk_is_embedded_in_the_specs_that_use_its_directory() {
    let registry = LanguageRegistry::builtin();
    for (dir, ids) in QUERY_DIRS {
        let Ok(entries) = fs::read_dir(langs_dir().join(dir)) else {
            continue;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "scm") {
                continue;
            }
            let name = path.file_name().unwrap().to_str().unwrap();
            let file = QueryFile::from_file_name(name)
                .unwrap_or_else(|| panic!("{dir}/{name} is not a known query file"));
            let text = fs::read_to_string(&path).unwrap();
            for id in ids {
                let spec = registry.get(&(*id).into()).unwrap().spec();
                let embedded = spec.source(file).unwrap_or_default();
                assert!(
                    embedded.contains(text.trim()),
                    "{dir}/{name} is not embedded in the {id} spec"
                );
            }
        }
    }
}

#[test]
fn every_language_has_tables_that_parse() {
    let registry = LanguageRegistry::builtin();
    for language in registry.iter() {
        // Reading the tables forces the one-time parse; a malformed file panics here.
        let _ = language.spec().tables();
    }
}

#[test]
fn the_contract_rejects_unknown_and_missing_captures() {
    let problems = contract(QueryFile::Symbols)
        .check(&["symbol.function", "symbol.nmae", "_ignored"])
        .unwrap_err();
    assert!(
        problems.iter().any(|p| p.contains("symbol.nmae")),
        "{problems:?}"
    );
    assert!(
        problems.iter().any(|p| p.contains("symbol.name")),
        "{problems:?}"
    );
    assert!(
        contract(QueryFile::Symbols)
            .check(&["symbol.method", "symbol.name", "_t"])
            .is_ok()
    );
}
