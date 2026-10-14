// ABOUTME: The matrix test (design §8.3.7, plan §2.8): enumerates checks × languages from the registries
// ABOUTME: and requires goldens, skips, valid evidence, declared-facts-only runs, and each check's fixtures (ADR-043, ADR-074).
use std::fs;
use std::path::Path;

use cupel_core::{CheckId, LangId, Support};
use cupel_fixtures::matrix::{Registries, fixture_repo, run_with_declared_facts, verify};
use cupel_fixtures::{GoldenMode, Goldens};

#[test]
fn every_pair_has_its_golden_or_its_skip_and_no_golden_is_stale() {
    let problems = verify(&Goldens::from_env());
    assert!(problems.is_empty(), "\n{}", problems.join("\n\n"));
}

#[test]
fn every_check_runs_given_only_its_declared_facts() {
    let registries = Registries::builtin();
    let supported: Vec<_> = registries
        .fixture_pairs()
        .into_iter()
        .filter(|pair| pair.support == Support::Supported)
        .collect();
    assert!(!supported.is_empty());
    for pair in supported {
        if let Err(problem) = run_with_declared_facts(&pair) {
            panic!("{}: {problem}", pair.golden());
        }
    }
}

/// Copies `from` (a directory of goldens) into `into`.
fn copy_dir(from: &Path, into: &Path) {
    fs::create_dir_all(into).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = into.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn deleting_a_golden_fails_the_matrix_and_names_it() {
    let registries = Registries::builtin();
    let pair = registries
        .pairs()
        .into_iter()
        .find(|pair| pair.support == Support::Supported)
        .expect("at least one supported pair");
    let dir = tempfile::tempdir().unwrap();
    copy_dir(Goldens::from_env().dir(), dir.path());
    fs::remove_file(dir.path().join(format!("{}.json", pair.golden()))).unwrap();
    fs::write(dir.path().join("static.complexity/cobol.json"), "{}\n").unwrap();

    let problems = verify(&Goldens::new(dir.path(), GoldenMode::Compare));

    let missing = format!("{}: golden", pair.golden());
    assert!(
        problems
            .iter()
            .any(|p| p.starts_with(&missing) && p.contains("does not exist")),
        "{problems:#?}"
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("static.complexity/cobol.json is stale")),
        "{problems:#?}"
    );
}

#[test]
fn a_check_runs_on_its_own_fixture_repository_where_it_has_one() {
    let repos = tempfile::tempdir().unwrap();
    fs::create_dir_all(repos.path().join("x.own/rust")).unwrap();
    let (own, other) = (CheckId::from("x.own"), CheckId::from("x.other"));
    let (rust, go) = (LangId::from("rust"), LangId::from("go"));

    assert_eq!(fixture_repo(repos.path(), &own, &rust), "x.own/rust");
    assert_eq!(fixture_repo(repos.path(), &own, &go), "go");
    assert_eq!(fixture_repo(repos.path(), &other, &rust), "rust");
}
