// ABOUTME: Golden outputs for every (language, syntactic fact) with a registered producer: the full
// ABOUTME: fact over the language's fixture repository, at `golden/facts/<lang>/<fact>.json`.
use std::fs;
use std::path::Path;

use cupel_core::{FactKind, ProducerRegistry};
use cupel_fixtures::Goldens;
use cupel_fixtures::conformance::produce_fixture_fact;
use cupel_lang::LanguageRegistry;

/// Every (language, fact) pair that has a producer, as golden names.
fn produced_pairs(languages: &LanguageRegistry, producers: &ProducerRegistry) -> Vec<String> {
    let mut names = Vec::new();
    for lang in languages.langs() {
        for kind in FactKind::SYNTACTIC {
            if producers.producers_for(kind, &lang).next().is_some() {
                names.push(format!("facts/{}/{kind}", lang.id));
            }
        }
    }
    names
}

#[test]
fn every_produced_fact_matches_its_golden() {
    let languages = LanguageRegistry::builtin();
    let mut producers = ProducerRegistry::default();
    languages.register_producers(&mut producers);
    let goldens = Goldens::from_env();
    let mut failures = Vec::new();
    for lang in languages.langs() {
        for kind in FactKind::SYNTACTIC {
            if producers.producers_for(kind, &lang).next().is_none() {
                continue;
            }
            let name = format!("facts/{}/{kind}", lang.id);
            match produce_fixture_fact(&producers, &languages, &lang, kind) {
                Ok(value) => {
                    if let Err(error) = goldens.check(&name, &value) {
                        failures.push(error.to_string());
                    }
                }
                Err(message) => failures.push(format!("{name}: {message}")),
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn every_fact_golden_belongs_to_a_registered_producer() {
    let languages = LanguageRegistry::builtin();
    let mut producers = ProducerRegistry::default();
    languages.register_producers(&mut producers);
    let expected = produced_pairs(&languages, &producers);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("golden/facts");
    for lang in fs::read_dir(&dir).into_iter().flatten().flatten() {
        for golden in fs::read_dir(lang.path()).into_iter().flatten().flatten() {
            let path = golden.path();
            let name = format!(
                "facts/{}/{}",
                lang.file_name().to_string_lossy(),
                path.file_stem().unwrap().to_string_lossy()
            );
            assert!(expected.contains(&name), "{} is stale", path.display());
        }
    }
}
