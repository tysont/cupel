// ABOUTME: A producer plugin registered in a fixture's cupel.toml flips its (check, language) pair from
// ABOUTME: `unsupported` to `supported` in `describe matrix`, and the matrix test then demands its golden (ADR-090).
use std::fs;
use std::path::Path;

use cupel_core::{CheckId, LangId, Support};
use cupel_fixtures::matrix::{Pair, Registries};
use cupel_fixtures::{copy_repo, run_cupel};
use serde_json::{Value, json};

const FIXTURE: &str = "ref.untested-python/typescript";

/// Whether `python3`, which runs the Python reference plugins, is installed; says so when not.
fn python() -> bool {
    let found = cupel_core::find_program("python3").is_some();
    if !found {
        eprintln!("SKIPPED: python3 not installed");
    }
    found
}

/// The `support` `describe matrix` reports for the Python check plugin on TypeScript in `dir`.
fn support_in(dir: &Path) -> Value {
    let outcome = run_cupel(&["describe", "matrix", "--format", "json"], dir);
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let described: Value = serde_json::from_str(&outcome.stdout).unwrap();
    described["description"]["cells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cell| cell["check"] == "ref.untested-python" && cell["lang"] == "typescript")
        .map_or_else(
            || panic!("no cell for the plugin check:\n{}", outcome.stdout),
            |cell| cell["support"].clone(),
        )
}

#[test]
fn a_producer_plugin_flips_its_pair_from_unsupported_to_supported_in_describe_matrix() {
    if !python() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo(FIXTURE, dir.path());
    assert_eq!(support_in(dir.path()), json!({"status": "supported"}));

    let config = dir.path().join("cupel.toml");
    let text = fs::read_to_string(&config).unwrap();
    let without_producer: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with("ref-produce"))
        .collect();
    fs::write(&config, without_producer.join("\n")).unwrap();

    assert_eq!(
        support_in(dir.path()),
        json!({"status": "unsupported", "fact": "per-test-map"})
    );
}

#[test]
fn the_matrix_test_demands_a_golden_for_the_pair_the_plugin_supports() {
    if !python() {
        return;
    }
    let pairs = Registries::builtin().fixture_pairs();
    let pair = Pair {
        check: CheckId::from("ref.untested-python"),
        lang: LangId::from("typescript"),
        support: Support::Supported,
    };
    assert!(pairs.contains(&pair), "{pairs:#?}");
    assert_eq!(pair.golden(), "ref.untested-python/typescript");
    assert_eq!(pair.fixture(), FIXTURE);
    assert!(
        !pairs
            .iter()
            .any(|other| other.check == pair.check && other.lang != pair.lang),
        "a plugin check has pairs only where it has its own fixture"
    );
}
