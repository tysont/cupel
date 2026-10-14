// ABOUTME: Integration tests for the most basic `cupel` invocations.
// ABOUTME: Covers the version string and the usage-error exit code for missing or unknown verbs.
mod support;

use predicates::prelude::*;
use support::cupel;

#[test]
fn version_prints_name_and_semver() {
    cupel()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^cupel \d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?\n$").unwrap());
}

#[test]
fn version_matches_the_crate_version() {
    cupel()
        .arg("--version")
        .assert()
        .stdout(format!("cupel {}\n", env!("CARGO_PKG_VERSION")));
}

/// Usage goes to stderr for people; stdout carries the usage-error envelope (ADR-018).
#[test]
fn no_arguments_prints_usage_to_stderr_and_exits_2() {
    cupel()
        .assert()
        .code(2)
        .stdout(predicate::str::contains("\"kind\": \"cli.usage\""))
        .stderr(predicate::str::contains("Usage: cupel"));
}

#[test]
fn unknown_verb_exits_2() {
    cupel()
        .arg("frobnicate")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("frobnicate"));
}
