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

/// The `paths` that `exclude` excludes, in order.
fn excluded<'a>(exclude: &Exclude, paths: &[&'a str]) -> Vec<&'a str> {
    paths
        .iter()
        .copied()
        .filter(|path| exclude.excludes(path))
        .collect()
}

#[test]
fn a_pattern_with_a_slash_is_anchored_at_the_repository_root() {
    // As in gitignore, `dir/**` matches what is inside the directory, not the directory.
    let paths = [
        "crates/cupel-fixtures/repos/go/ledger.go",
        "crates/cupel-fixtures/src/lib.rs",
        "crates/cupel-fixtures",
        "crates/cupel-core/src/lib.rs",
        "vendored/crates/cupel-fixtures/src/lib.rs",
    ];
    assert_eq!(
        excluded(&exclude(&["crates/cupel-fixtures/**"]), &paths),
        paths[..2]
    );
}

#[test]
fn an_excluded_directory_excludes_everything_below_it() {
    let paths = [
        "crates/cupel-fixtures/repos/rust/src/lib.rs",
        "crates/cupel-fixtures-extra/src/lib.rs",
    ];
    assert_eq!(
        excluded(&exclude(&["crates/cupel-fixtures"]), &paths),
        paths[..1]
    );
}

#[test]
fn a_pattern_without_a_slash_matches_a_name_at_any_depth() {
    let paths = [
        "generated/a.rs",
        "src/generated/a.rs",
        "api/v1/service.pb.go",
        "api/v1/service.go",
    ];
    assert_eq!(
        excluded(&exclude(&["generated", "*.pb.go"]), &paths),
        paths[..3]
    );
}

#[test]
fn a_star_does_not_cross_a_directory_separator() {
    let paths = ["src/a.rs", "src/deep/a.rs"];
    assert_eq!(excluded(&exclude(&["src/*.rs"]), &paths), paths[..1]);
}

#[test]
fn vendored_dependencies_build_output_and_cupels_own_directory_are_always_excluded() {
    let paths = [
        "vendor/github.com/x/y.go",
        "web/node_modules/left-pad/index.js",
        "target/debug/build/out.rs",
        ".cupel/work/run/base/src/lib.rs",
        "src/vendors.rs",
        "src/targeting/mod.rs",
    ];
    assert_eq!(excluded(&Exclude::default(), &paths), paths[..4]);
}

#[test]
fn an_invalid_glob_is_a_configuration_error_naming_the_pattern() {
    let error = ConfigFile::parse("[scope]\nexclude = [\"src/[\"]\n", Path::new("cupel.toml"))
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "invalid configuration in `cupel.toml`: TOML parse error at line 2, column 11\n  |\n\
         2 | exclude = [\"src/[\"]\n  |           ^^^^^^^^^\n\
         invalid exclude pattern `src/[`: error parsing glob 'src/[': unclosed character class; \
         missing ']'"
    );
}

#[test]
fn the_scope_table_holds_exclude_and_rejects_other_keys() {
    let file = ConfigFile::parse(
        "[scope]\nexclude = [\"crates/cupel-fixtures/**\"]\n",
        Path::new("cupel.toml"),
    )
    .unwrap();
    assert_eq!(file.scope.exclude.patterns(), ["crates/cupel-fixtures/**"]);
    let error = ConfigFile::parse("[scope]\ninclude = []\n", Path::new("cupel.toml"))
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "invalid configuration in `cupel.toml`: TOML parse error at line 2, column 1\n  |\n\
         2 | include = []\n  | ^^^^^^^\nunknown field `include`, expected `exclude`"
    );
}
