// ABOUTME: A tools.deps Clojure project whose pom.xml is a template without coordinates and whose Java sources
// ABOUTME: its tools.build compiles: `init` makes it one Clojure root, with no Maven root or Java runner.
use std::fs;

use cupel_fixtures::{copy_repo, run_cupel};

/// The `cupel.toml` that `init --write` writes for a copy of the fixture.
fn initialized_config() -> toml::Value {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("tools-build", dir.path());
    let outcome = run_cupel(&["init", "--write", "--format", "json"], dir.path());
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    toml::from_str(&fs::read_to_string(dir.path().join("cupel.toml")).unwrap()).unwrap()
}

/// The keys of the table at `key` in `config`, empty without one.
fn keys(config: &toml::Value, key: &str) -> Vec<String> {
    config
        .get(key)
        .and_then(toml::Value::as_table)
        .map(|table| table.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn init_writes_no_java_runner_for_a_pom_template_without_coordinates() {
    let config = initialized_config();
    assert_eq!(keys(&config, "lang"), ["clojure"]);
    assert_eq!(keys(&config, "roots"), Vec::<String>::new());
}
