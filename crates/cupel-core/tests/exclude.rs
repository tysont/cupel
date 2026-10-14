// ABOUTME: Tests for `[scope] exclude`: gitignore-style globs that remove paths (and everything below
// ABOUTME: an excluded directory) from a run, plus the directory names every run excludes (ADR-065).
use std::path::Path;

use cupel_core::Exclude;
use cupel_core::config::ConfigFile;

fn exclude(patterns: &[&str]) -> Exclude {
    Exclude::new(
        patterns
            .iter()
            .map(|pattern| (*pattern).to_owned())
            .collect(),
    )
    .unwrap()
}

#[test]
fn a_pattern_with_a_slash_is_anchored_at_the_repository_root() {
    let exclude = exclude(&["crates/cupel-fixtures/**"]);
    assert!(exclude.excludes("crates/cupel-fixtures/repos/go/ledger.go"));
    assert!(exclude.excludes("crates/cupel-fixtures/src/lib.rs"));
    assert!(
        !exclude.excludes("crates/cupel-fixtures"),
        "as in gitignore, `dir/**` matches what is inside the directory"
    );
    assert!(!exclude.excludes("crates/cupel-core/src/lib.rs"));
    assert!(!exclude.excludes("vendored/crates/cupel-fixtures/src/lib.rs"));
}

#[test]
fn an_excluded_directory_excludes_everything_below_it() {
    let exclude = exclude(&["crates/cupel-fixtures"]);
    assert!(exclude.excludes("crates/cupel-fixtures/repos/rust/src/lib.rs"));
    assert!(!exclude.excludes("crates/cupel-fixtures-extra/src/lib.rs"));
}

#[test]
fn a_pattern_without_a_slash_matches_a_name_at_any_depth() {
    let exclude = exclude(&["generated", "*.pb.go"]);
    assert!(exclude.excludes("generated/a.rs"));
    assert!(exclude.excludes("src/generated/a.rs"));
    assert!(exclude.excludes("api/v1/service.pb.go"));
    assert!(!exclude.excludes("api/v1/service.go"));
}

#[test]
fn a_star_does_not_cross_a_directory_separator() {
    let exclude = exclude(&["src/*.rs"]);
    assert!(exclude.excludes("src/a.rs"));
    assert!(!exclude.excludes("src/deep/a.rs"));
}

#[test]
fn vendored_dependencies_build_output_and_cupels_own_directory_are_always_excluded() {
    let exclude = Exclude::default();
    for path in [
        "vendor/github.com/x/y.go",
        "web/node_modules/left-pad/index.js",
        "target/debug/build/out.rs",
        ".cupel/work/run/base/src/lib.rs",
    ] {
        assert!(exclude.excludes(path), "{path}");
    }
    assert!(!exclude.excludes("src/vendors.rs"));
    assert!(!exclude.excludes("src/targeting/mod.rs"));
}

#[test]
fn an_invalid_glob_is_a_configuration_error_naming_the_pattern() {
    let error = ConfigFile::parse("[scope]\nexclude = [\"src/[\"]\n", Path::new("cupel.toml"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("`src/[`"), "{error}");
    assert!(error.contains("cupel.toml"), "{error}");
}

#[test]
fn the_scope_table_holds_exclude_and_rejects_other_keys() {
    let file = ConfigFile::parse(
        "[scope]\nexclude = [\"crates/cupel-fixtures/**\"]\n",
        Path::new("cupel.toml"),
    )
    .unwrap();
    assert_eq!(file.scope.exclude.patterns(), ["crates/cupel-fixtures/**"]);
    assert!(
        file.scope
            .exclude
            .excludes("crates/cupel-fixtures/src/lib.rs")
    );
    let error = ConfigFile::parse("[scope]\ninclude = []\n", Path::new("cupel.toml"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("exclude"), "{error}");
}
