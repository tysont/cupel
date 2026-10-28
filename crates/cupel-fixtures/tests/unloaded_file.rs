// ABOUTME: `tests.coverage` on a Python package with a module no test imports (ADR-152): run as v0.2.0 configured it,
// ABOUTME: the file is `coverage.unloaded-file`; run with the source `init` now passes, coverage.py lists it uncovered.
use std::fs;
use std::path::Path;

use cupel_fixtures::{copy_repo, run_cupel};
use serde_json::{Value, json};

/// The two functions of `src/shop/shipping.py`, which `tests/test_pricing.py` never imports.
const SHIPPING: [&str; 2] = ["shop.shipping.shipping_cost", "shop.shipping.free_shipping"];

/// A copy of the fixture, with its committed report.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("unloaded-file/python", dir.path());
    dir
}

/// `check --only tests.coverage` in `dir`, exit 0, as JSON.
fn check(dir: &Path) -> Value {
    let outcome = run_cupel(
        &["check", "--only", "tests.coverage", "--format", "json"],
        dir,
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    serde_json::from_str(&outcome.stdout).unwrap()
}

/// Each finding as `(kind, symbol, evidence)`.
fn found(envelope: &Value) -> Vec<(String, String, Value)> {
    envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            (
                finding["kind"].as_str().unwrap().to_owned(),
                finding["symbol"]["display"].as_str().unwrap().to_owned(),
                finding["evidence"].clone(),
            )
        })
        .collect()
}

/// With the bare `--cov` v0.2.0 wrote, coverage.py lists only the files the tests loaded, and the
/// check reports each function of the file it never loaded with its lines all uncovered.
#[test]
fn a_file_no_test_loads_is_reported_unloaded_with_none_of_its_lines_covered() {
    let dir = fixture();
    let envelope = check(dir.path());
    let expected: Vec<(String, String, Value)> = [(5, SHIPPING[0]), (2, SHIPPING[1])]
        .into_iter()
        .map(|(lines, symbol)| {
            (
                "coverage.unloaded-file".to_owned(),
                symbol.to_owned(),
                json!({"lines": lines, "covered": 0}),
            )
        })
        .collect();
    assert_eq!(found(&envelope), expected);
    let result = &envelope["checks"][0];
    assert_eq!(result["status"], json!("fail"));
    assert_eq!(result["metrics"]["uncovered"], json!(2.0));
    assert_eq!(
        result["metrics"]["lines"],
        json!(9.0),
        "pricing's two instrumented lines and shipping's seven span lines"
    );
    let outcome = run_cupel(
        &["check", "--only", "tests.coverage", "--format", "json"],
        dir.path(),
    );
    outcome.assert_golden("unloaded-file/python");
}

/// With `--cov=src`, the source `cupel init` now passes, coverage.py lists `shipping.py` at 0 %
/// and the same functions are ordinary uncovered symbols; needs pytest with pytest-cov.
#[test]
fn with_a_source_the_file_is_listed_and_its_functions_are_uncovered_symbols() {
    if cupel_core::find_program("pytest").is_none() {
        eprintln!("SKIPPED: pytest not installed; the --cov=src run was not made");
        return;
    }
    let dir = fixture();
    fs::remove_dir_all(dir.path().join(".cupel")).unwrap();
    fs::write(
        dir.path().join("cupel.toml"),
        "[lang.python]\ncoverage = \"PYTHONDONTWRITEBYTECODE=1 pytest -p no:cacheprovider --cov=src --cov-report=lcov:{out}\"\n",
    )
    .unwrap();
    let envelope = check(dir.path());
    let kinds: Vec<(String, String)> = found(&envelope)
        .into_iter()
        .map(|(kind, symbol, _)| (kind, symbol))
        .collect();
    assert_eq!(
        kinds,
        SHIPPING.map(|symbol| ("coverage.uncovered-symbol".to_owned(), symbol.to_owned()))
    );
    assert_eq!(envelope["checks"][0]["status"], json!("fail"));
}
