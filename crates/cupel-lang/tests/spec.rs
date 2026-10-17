// ABOUTME: Tests for language specs: every embedded query compiles against every grammar of its
// ABOUTME: language, uses only contract capture names, and every `.scm` on disk is embedded.
use std::fs;
use std::path::{Path, PathBuf};

use cupel_lang::{LanguageRegistry, QueryFile, contract};

fn langs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/langs")
}

#[test]
fn every_query_compiles_against_every_grammar_of_its_language_with_contract_captures() {
    let registry = LanguageRegistry::builtin();
    // Each language's grammars, through one path per extension it claims.
    let problems: Vec<String> = registry
        .iter()
        .flat_map(|language| {
            let id = language.id();
            let paths: Vec<String> = language
                .extensions()
                .iter()
                .map(|extension| format!("a{extension}"))
                .collect();
            QueryFile::ALL
                .into_iter()
                .filter(|file| language.spec().source(*file).is_some())
                .flat_map(move |file| paths.clone().into_iter().map(move |path| (file, path)))
                .filter_map(|(file, path)| {
                    let grammar = language.grammar(Path::new(&path));
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
    let missing: Vec<String> = query_dirs(registry)
        .iter()
        .flat_map(|(dir, ids)| {
            query_files(dir).into_iter().flat_map(move |path| {
                let name = path.file_name().unwrap().to_str().unwrap().to_owned();
                let file = QueryFile::from_file_name(&name)
                    .unwrap_or_else(|| panic!("{dir}/{name} is not a known query file"));
                let text = fs::read_to_string(&path).unwrap();
                ids.iter()
                    .filter(move |id| {
                        let spec = registry.get(&id.as_str().into()).unwrap().spec();
                        !spec.source(file).unwrap_or_default().contains(text.trim())
                    })
                    .map(move |id| format!("{dir}/{name} is not embedded in the {id} spec"))
            })
        })
        .collect();
    assert_eq!(missing, Vec::<String>::new());
}

/// Every query directory on disk with the languages whose specs embed it: `<dir>/queries` is the
/// language `<dir>`'s, or, for a directory several languages share (`ecma`), those languages
/// with a subdirectory of their own under it; `<dir>/queries/<id>` is the language `<id>`'s.
fn query_dirs(registry: &LanguageRegistry) -> Vec<(String, Vec<String>)> {
    let ids: Vec<String> = registry.ids().iter().map(ToString::to_string).collect();
    let mut dirs = Vec::new();
    for entry in fs::read_dir(langs_dir()).unwrap() {
        let path = entry.unwrap().path();
        let dir = path.file_name().unwrap().to_str().unwrap().to_owned();
        if !path.join("queries").is_dir() {
            continue;
        }
        let own: Vec<String> = ids
            .iter()
            .filter(|id| path.join("queries").join(id.as_str()).is_dir())
            .cloned()
            .collect();
        let shared = if ids.contains(&dir) {
            vec![dir.clone()]
        } else {
            own.clone()
        };
        dirs.push((format!("{dir}/queries"), shared));
        for id in own {
            dirs.push((format!("{dir}/queries/{id}"), vec![id]));
        }
    }
    dirs.sort();
    dirs
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
    let without_framework: Vec<String> = registry
        .iter()
        .filter(|language| language.spec().tables().tests.framework.is_empty())
        .map(|language| language.id().to_string())
        .collect();
    assert_eq!(
        without_framework,
        Vec::<String>::new(),
        "every language names its default test framework"
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
