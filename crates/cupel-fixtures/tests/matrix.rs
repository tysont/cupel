// ABOUTME: The matrix test (design §8.3.7, plan §2.8): enumerates checks × languages from the registries
// ABOUTME: and requires goldens, skips, valid evidence, and declared-facts-only runs (ADR-043).
use std::fs;
use std::path::Path;

use cupel_core::Support;
use cupel_fixtures::matrix::{Registries, run_with_declared_facts, verify};
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
        .pairs()
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
