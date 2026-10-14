// ABOUTME: Tests for finding and loading `cupel.toml` on a real file system: an explicit `--config`
// ABOUTME: path, or the nearest file walking up from the root; core never consults the process cwd.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::config::{self, Config, ConfigError, ConfigRequest, Settings};
use cupel_core::{ConfigSource, CostClass, Error};

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A temporary directory holding a `cupel.toml` of `text`.
fn with_config(text: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("cupel.toml"), text);
    dir
}

fn request(root: &Path, config_path: Option<PathBuf>) -> ConfigRequest {
    ConfigRequest {
        root: root.to_owned(),
        config_path,
        profile: None,
        flags: Settings::default(),
        env: BTreeMap::new(),
    }
}

#[test]
fn finds_the_file_in_the_root() {
    let dir = with_config("");
    assert_eq!(
        config::discover(dir.path(), None).unwrap(),
        Some(dir.path().join("cupel.toml"))
    );
}

#[test]
fn walks_up_to_the_nearest_file() {
    let dir = tempfile::tempdir().unwrap();
    let outer = dir.path().join("cupel.toml");
    let inner = dir.path().join("a/cupel.toml");
    write(&outer, "depth = \"deep\"\n");
    write(&inner, "depth = \"quick\"\n");
    let deep_root = dir.path().join("a/b/c");
    fs::create_dir_all(&deep_root).unwrap();

    assert_eq!(
        config::discover(&deep_root, None).unwrap(),
        Some(inner.clone())
    );
    assert_eq!(
        config::discover(&dir.path().join("a"), None).unwrap(),
        Some(inner)
    );
    let sibling = dir.path().join("z");
    fs::create_dir_all(&sibling).unwrap();
    assert_eq!(config::discover(&sibling, None).unwrap(), Some(outer));
}

#[test]
fn a_directory_named_cupel_toml_is_not_a_config_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir_all(root.join("cupel.toml")).unwrap();
    write(&dir.path().join("cupel.toml"), "");
    assert_eq!(
        config::discover(&root, None).unwrap(),
        Some(dir.path().join("cupel.toml"))
    );
}

#[test]
fn explicit_path_wins_and_relative_paths_resolve_against_the_root() {
    let dir = with_config("");
    let custom = dir.path().join("conf/ci.toml");
    write(&custom, "");

    assert_eq!(
        config::discover(dir.path(), Some(Path::new("conf/ci.toml"))).unwrap(),
        Some(custom.clone())
    );
    assert_eq!(
        config::discover(Path::new("/elsewhere"), Some(&custom)).unwrap(),
        Some(custom)
    );
}

#[test]
fn missing_explicit_path_is_a_usage_error() {
    let dir = with_config("");
    let err = config::discover(dir.path(), Some(Path::new("nope.toml"))).unwrap_err();
    let missing = dir.path().join("nope.toml");
    assert_eq!(
        err,
        Error::Config(ConfigError::Missing {
            path: missing.clone()
        })
    );
    assert_eq!(
        err.to_string(),
        format!(
            "config file `{}` does not exist; fix the --config path",
            missing.display()
        )
    );
}

#[test]
fn load_reads_and_resolves_the_discovered_file() {
    let dir = with_config("depth = \"deep\"\n");
    let root = dir.path().join("src/module");
    fs::create_dir_all(&root).unwrap();

    let config = Config::load(&request(&root, None)).unwrap();
    assert_eq!(config.path, Some(dir.path().join("cupel.toml")));
    assert_eq!(config.effective.depth, CostClass::Deep);
    assert_eq!(
        config.sources,
        [ConfigSource::CupelToml, ConfigSource::Defaults]
    );
}

#[test]
fn load_without_a_file_uses_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::load(&request(dir.path(), None)).unwrap();
    // A cupel.toml in an ancestor of the temp dir would make this test meaningless.
    assert_eq!(config::discover(dir.path(), None).unwrap(), None);
    assert_eq!(config.path, None);
    assert_eq!(config.effective.depth, CostClass::Standard);
    assert_eq!(config.sources, [ConfigSource::Defaults]);
}

#[test]
fn load_reports_parse_errors_against_the_file_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cupel.toml");
    write(&path, "colour = \"never\"\n");
    let err = Config::load(&request(dir.path(), None)).unwrap_err();
    let Error::Config(ConfigError::Invalid {
        path: err_path,
        message,
    }) = err
    else {
        panic!("unexpected error {err:?}");
    };
    assert_eq!(err_path, path);
    assert!(message.contains("unknown field `colour`"), "{message}");
}
