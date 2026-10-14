// ABOUTME: `static.crap` through the binary on the fixture repositories: the skip without `no_coverage`,
// ABOUTME: `describe` of the check and its kinds, `explain` with excerpt and test hint, one-line text evidence.
use std::fs;

use cupel_fixtures::{copy_repo, run_cupel};
use serde_json::{Value, json};

/// A copy of the `lang` fixture repository.
fn fixture(lang: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo(lang, dir.path());
    dir
}

/// Runs `cupel <args>` in `dir`, requiring exit 0; stdout as JSON.
fn json(dir: &std::path::Path, args: &[&str]) -> Value {
    let outcome = run_cupel(args, dir);
    assert_eq!(outcome.code, Some(0), "{args:?}: {}", outcome.stdout);
    serde_json::from_str(&outcome.stdout).unwrap()
}

#[test]
fn without_no_coverage_the_check_is_skipped_saying_how_to_run_it() {
    let dir = fixture("rust");
    fs::remove_file(dir.path().join("cupel.toml")).unwrap();
    let envelope = json(
        dir.path(),
        &["check", "--only", "static.crap", "--format", "json"],
    );
    assert_eq!(envelope["status"], json!("pass"));
    assert_eq!(envelope["findings"], json!([]));
    let checks = envelope["checks"].as_array().unwrap();
    assert_eq!(checks.len(), 1, "{checks:#?}");
    assert_eq!(checks[0]["id"], json!("static.crap"));
    assert_eq!(checks[0]["status"], json!("skipped"));
    assert_eq!(
        checks[0]["skipped"],
        json!({
            "reason": "unconfigured",
            "fact": "coverage-map",
            "lang": "rust",
            "tool": null,
            "see": "cupel describe static.crap"
        })
    );
}

#[test]
fn describe_shows_the_check_and_its_finding_kinds_from_the_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let check = json(dir.path(), &["describe", "static.crap"])["description"].clone();
    let manifest = &check["manifest"];
    assert_eq!(
        manifest["needs"],
        json!(["symbols", "decisions", "norm-tree"])
    );
    assert_eq!(
        manifest["wants"],
        json!(["coverage-map", "test-conventions"])
    );
    let metrics: Vec<(&str, &str)> = manifest["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|metric| {
            (
                metric["name"].as_str().unwrap(),
                metric["per"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        metrics,
        [("max", "scope"), ("mean", "scope"), ("crap", "symbol")]
    );
    let high = json(dir.path(), &["describe", "finding", "crap.high"])["description"].clone();
    assert_eq!(high["check"], json!("static.crap"));
    assert_eq!(high["action"], json!("write_test"));
    let properties = high["evidence"]["properties"].as_object().unwrap();
    let mut fields: Vec<&str> = properties.keys().map(String::as_str).collect();
    fields.sort_unstable();
    assert_eq!(fields, ["coverage", "crap", "cyclomatic", "high", "watch"]);
    let watch = json(dir.path(), &["describe", "finding", "crap.watch"])["description"].clone();
    assert_eq!(watch["action"], json!("review"));
}

#[test]
fn explain_shows_the_excerpt_evidence_and_nearest_test_file_of_a_high_finding() {
    let dir = fixture("javascript");
    let envelope = json(
        dir.path(),
        &["check", "--only", "static.crap", "--format", "json"],
    );
    let finding = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["symbol"]["display"] == json!("decide.classify"))
        .expect("decide.classify is crap.high")
        .clone();
    assert_eq!(finding["kind"], json!("crap.high"));
    let id = finding["id"].as_str().unwrap();
    let explained = json(dir.path(), &["explain", id, "--format", "json"]);
    let explanation = &explained["explanation"];
    assert_eq!(
        explanation["nearest_test_file"],
        json!("test/decide.test.js")
    );
    let excerpt = &explanation["excerpt"];
    assert_eq!(excerpt["file"], json!("src/decide.js"));
    assert_eq!(
        excerpt["lines"][0]["text"],
        json!("export function classify(balance, limit) {")
    );
    assert_eq!(
        explained["findings"][0]["evidence"],
        json!({"crap": 30.0, "cyclomatic": 5, "coverage": null, "watch": 5, "high": 30})
    );
}

#[test]
fn text_output_summarizes_the_evidence_on_one_line() {
    let dir = fixture("go");
    let outcome = run_cupel(
        &["check", "--only", "static.crap", "--format", "text"],
        dir.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let lines: Vec<&str> = outcome.stdout.lines().collect();
    let title = lines
        .iter()
        .position(|line| line.contains("`ledger.Overdue` has CRAP 42.0"))
        .expect("the Overdue finding is listed");
    let evidence = lines[title..]
        .iter()
        .find(|line| line.trim_start().starts_with("evidence:"))
        .unwrap();
    assert!(
        evidence.contains("crap 42.0 · cyclomatic 6 · high 30 · watch 5"),
        "{evidence}"
    );
}
