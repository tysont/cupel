// ABOUTME: `static.crap` through the binary: no CRAP without coverage, CRAP from the coverage fixtures' reports with
// ABOUTME: `explain` and one-line text evidence, and `describe` of the check and its kinds (ADR-067, ADR-077).
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

#[test]
fn with_coverage_explain_shows_the_excerpt_evidence_and_nearest_test_file_of_a_high_finding() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("coverage/javascript", dir.path());
    let envelope = json(
        dir.path(),
        &["check", "--only", "static.crap", "--format", "json"],
    );
    let finding = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["symbol"]["display"] == json!("grade.refund"))
        .expect("grade.refund is crap.high")
        .clone();
    assert_eq!(finding["kind"], json!("crap.high"));
    let id = finding["id"].as_str().unwrap();
    let explained = json(dir.path(), &["explain", id, "--format", "json"]);
    let explanation = &explained["explanation"];
    assert_eq!(explanation["nearest_test_file"], json!("grade.test.js"));
    assert_eq!(explanation["excerpt"]["file"], json!("grade.js"));
    assert_eq!(
        explained["findings"][0]["evidence"],
        json!({"crap": 42.0, "cyclomatic": 6, "coverage": 0.0, "watch": 5, "high": 30})
    );
}

#[test]
fn with_coverage_text_output_summarizes_the_evidence_on_one_line() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("coverage/go", dir.path());
    let outcome = run_cupel(
        &["check", "--only", "static.crap", "--format", "text"],
        dir.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let lines: Vec<&str> = outcome.stdout.lines().collect();
    let title = lines
        .iter()
        .position(|line| line.contains("`grade.Refund` has CRAP 42.0"))
        .expect("the Refund finding is listed");
    let evidence = lines[title..]
        .iter()
        .find(|line| line.trim_start().starts_with("evidence:"))
        .unwrap();
    assert!(
        evidence.contains("coverage 0.0 · crap 42.0 · cyclomatic 6 · high 30 · watch 5"),
        "{evidence}"
    );
}
