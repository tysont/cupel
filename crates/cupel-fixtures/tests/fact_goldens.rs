// ABOUTME: Golden outputs for every (language, syntactic fact) with a registered producer: the full fact over the
// ABOUTME: language's fixture repository, at `golden/facts/<lang>/<fact>.json`, from cold trees and from reparsed ones.
use std::fs;
use std::path::Path;
use std::sync::Arc;

use cupel_core::syntax::ProjectInfo;
use cupel_core::{Cancellation, FactKind, ProducerRegistry};
use cupel_fixtures::conformance::{produce_fact_at, produce_fixture_fact};
use cupel_fixtures::{Goldens, copy_repo};
use cupel_lang::{LanguageRegistry, ParseCache, Reparse, walk};

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

/// Rewrites every file under `root` to `text(original)` and reparses it in `languages`' cache
/// from its previous tree; how many files reparsed.
fn rewrite_and_reparse(
    languages: &LanguageRegistry,
    root: &Path,
    files: &[String],
    text: impl Fn(&str) -> String,
    originals: &[String],
) -> usize {
    files
        .iter()
        .zip(originals)
        .filter(|(file, original)| {
            fs::write(root.join(file), text(original)).unwrap();
            languages.cache().reparse(&root.join(file)) == Reparse::Reparsed
        })
        .count()
}

/// The daemon's trees (ADR-085 §3, §10) give the facts cold trees give: every file of a copy of
/// each fixture is edited and reparsed from its tree, the original trees are swept away, and the
/// original text is reparsed back from the edited tree; every fact golden must still hold, read
/// from those trees alone.
#[test]
fn every_fact_golden_holds_for_trees_reparsed_from_edited_ones() {
    let goldens = Goldens::from_env();
    let mut failures = Vec::new();
    for lang in LanguageRegistry::builtin().langs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        copy_repo(lang.id.as_str(), &root);
        let languages = LanguageRegistry::with_cache(Arc::new(ParseCache::default()));
        let mut producers = ProducerRegistry::default();
        languages.register_producers(&mut producers);
        let files = walk(&root, &languages, &ProjectInfo::default());
        let originals: Vec<String> = files
            .iter()
            .map(|file| fs::read_to_string(root.join(file)).unwrap())
            .collect();
        languages.parse_all(&root, &files, &Cancellation::new());
        let edited = |text: &str| format!("\n{text}\n");
        let parsed = usize::try_from(languages.cache().stats().misses).unwrap();
        assert_eq!(
            rewrite_and_reparse(&languages, &root, &files, edited, &originals),
            parsed
        );
        languages.cache().sweep();
        let back = |text: &str| text.to_owned();
        assert_eq!(
            rewrite_and_reparse(&languages, &root, &files, back, &originals),
            parsed
        );
        let misses = languages.cache().stats().misses;
        for kind in FactKind::SYNTACTIC {
            if producers.producers_for(kind, &lang).next().is_none() {
                continue;
            }
            let name = format!("facts/{}/{kind}", lang.id);
            match produce_fact_at(&producers, &languages, &lang, kind, &root) {
                Ok(value) => {
                    if let Err(error) = goldens.check(&name, &value) {
                        failures.push(error.to_string());
                    }
                }
                Err(message) => failures.push(format!("{name}: {message}")),
            }
        }
        assert_eq!(
            languages.cache().stats().misses,
            misses,
            "{}: the facts were read from the reparsed trees",
            lang.id
        );
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
