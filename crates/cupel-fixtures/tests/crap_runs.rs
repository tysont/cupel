// ABOUTME: `static.crap` through the binary on the fixture repositories: without coverage it reports no CRAP,
// ABOUTME: and `describe` shows the check, its limitation, and its finding kinds from the manifest (ADR-067).
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
fn without_coverage_the_check_passes_with_no_crap_in_every_language() {
    for lang in ["go", "javascript", "python", "rust", "typescript"] {
        let dir = fixture(lang);
        let envelope = json(
            dir.path(),
            &["check", "--only", "static.crap", "--format", "json"],
        );
        assert_eq!(envelope["status"], json!("pass"), "{lang}");
        assert_eq!(envelope["findings"], json!([]), "{lang}");
        let checks = envelope["checks"].as_array().unwrap();
        assert_eq!(checks.len(), 1, "{lang}: {checks:#?}");
        assert_eq!(checks[0]["status"], json!("pass"), "{lang}");
        assert_eq!(checks[0]["score"], Value::Null, "{lang}");
        assert_eq!(checks[0]["metrics"], json!({}), "{lang}");
        assert_eq!(checks[0]["skipped"], Value::Null, "{lang}");
    }
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
    assert!(
        manifest["summary"]
            .as_str()
            .unwrap()
            .contains("CRAP exists only where coverage was measured"),
        "{manifest}"
    );
    let watch = json(dir.path(), &["describe", "finding", "crap.watch"])["description"].clone();
    assert_eq!(watch["action"], json!("review"));
}
