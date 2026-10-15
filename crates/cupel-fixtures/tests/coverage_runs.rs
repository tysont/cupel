// ABOUTME: `tests.coverage` through the binary on the coverage fixtures, whose committed reports must stay fresh:
// ABOUTME: findings per language, the Go note, `explain` of an uncovered function, and its one-line text evidence.
use cupel_core::{Availability, Ctx, FactKind, Lang, LangFamily, LangId, ProducerRegistry};
use cupel_fixtures::{copy_repo, repo_path, run_cupel};
use serde_json::{Value, json};

const LANGS: [&str; 5] = ["go", "javascript", "python", "rust", "typescript"];

/// A copy of the coverage fixture for `lang`, with its committed report.
fn fixture(lang: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo(&format!("coverage/{lang}"), dir.path());
    dir
}

/// Runs `cupel <args>` in `dir`, requiring exit 0; stdout as JSON.
fn json(dir: &std::path::Path, args: &[&str]) -> Value {
    let outcome = run_cupel(args, dir);
    assert_eq!(outcome.code, Some(0), "{args:?}: {}", outcome.stdout);
    serde_json::from_str(&outcome.stdout).unwrap()
}

fn check(dir: &std::path::Path) -> Value {
    json(
        dir,
        &["check", "--only", "tests.coverage", "--format", "json"],
    )
}

/// Each finding as `(kind, symbol)`.
fn found(envelope: &Value) -> Vec<(String, String)> {
    envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            (
                finding["kind"].as_str().unwrap().to_owned(),
                finding["symbol"]["display"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn every_committed_coverage_report_is_fresh() {
    let mut producers = ProducerRegistry::default();
    cupel_adapt::register(&mut producers);
    for (fixture, lang) in ["coverage", "tests.mutation"]
        .into_iter()
        .flat_map(|fixture| LANGS.map(|lang| (fixture, lang)))
    {
        let ctx = Ctx::new(repo_path(&format!("{fixture}/{lang}")));
        let language = Lang {
            id: LangId::from(lang),
            family: LangFamily::from(lang),
        };
        let ready = producers
            .producers_for(FactKind::CoverageMap, &language)
            .any(|producer| producer.availability(&ctx) == Availability::Ready);
        assert!(
            ready,
            "the coverage report committed in repos/{fixture}/{lang}/.cupel/cache/coverage/ is \
             stale: delete that directory, run `cupel check --only tests.coverage` in a copy of \
             the fixture with the tool installed, and commit the copy's new cache (ADR-079)"
        );
    }
}

#[test]
fn each_language_reports_its_uncovered_function_and_branches_where_measured() {
    let expected = [
        ("go", vec![("coverage.uncovered-symbol", "grade.Refund")]),
        (
            "javascript",
            vec![
                ("coverage.uncovered-branch", "grade.classify"),
                ("coverage.uncovered-symbol", "grade.refund"),
            ],
        ),
        (
            "python",
            vec![
                ("coverage.uncovered-branch", "grade.classify"),
                ("coverage.uncovered-symbol", "grade.refund"),
            ],
        ),
        ("rust", vec![("coverage.uncovered-symbol", "refund")]),
        (
            "typescript",
            vec![
                ("coverage.uncovered-branch", "grade.classify"),
                ("coverage.uncovered-symbol", "grade.refund"),
            ],
        ),
    ];
    for (lang, findings) in expected {
        let dir = fixture(lang);
        let envelope = check(dir.path());
        let findings: Vec<(String, String)> = findings
            .into_iter()
            .map(|(kind, symbol)| (kind.to_owned(), symbol.to_owned()))
            .collect();
        assert_eq!(found(&envelope), findings, "{lang}");
        assert_eq!(envelope["checks"][0]["status"], json!("fail"), "{lang}");
    }
}

#[test]
fn go_has_no_branch_findings_and_its_result_notes_branches_unavailable() {
    let dir = fixture("go");
    let envelope = check(dir.path());
    let result = &envelope["checks"][0];
    assert_eq!(result["notes"], json!(["branches: unavailable (go)"]));
    assert_eq!(result["metrics"].get("branch"), None);
    assert!(
        found(&envelope)
            .iter()
            .all(|(kind, _)| kind != "coverage.uncovered-branch")
    );
    let measured = check(fixture("python").path());
    assert_eq!(measured["checks"][0].get("notes"), None);
}

#[test]
fn explain_shows_the_uncovered_function_its_evidence_and_nearest_test_file() {
    let dir = fixture("javascript");
    let envelope = check(dir.path());
    let finding = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["kind"] == json!("coverage.uncovered-symbol"))
        .unwrap()
        .clone();
    let id = finding["id"].as_str().unwrap();
    let explained = json(dir.path(), &["explain", id, "--format", "json"]);
    let explanation = &explained["explanation"];
    assert_eq!(explanation["nearest_test_file"], json!("grade.test.js"));
    let excerpt = &explanation["excerpt"];
    assert_eq!(excerpt["file"], json!("grade.js"));
    let focus: Vec<&str> = excerpt["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|line| line["focus"] == json!(true))
        .map(|line| line["text"].as_str().unwrap())
        .collect();
    assert_eq!(focus[0], "function refund(amount, days, member) {");
    assert_eq!(
        explained["findings"][0]["evidence"],
        json!({"lines": 9, "covered": 0})
    );
}

#[test]
fn text_output_summarizes_the_evidence_and_the_note_on_one_line_each() {
    let dir = fixture("go");
    let outcome = run_cupel(
        &["check", "--only", "tests.coverage", "--format", "text"],
        dir.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let lines: Vec<&str> = outcome.stdout.lines().collect();
    let title = lines
        .iter()
        .position(|line| line.contains("`grade.Refund` never ran under test"))
        .expect("the Refund finding is listed");
    let evidence = lines[title..]
        .iter()
        .find(|line| line.trim_start().starts_with("evidence:"))
        .unwrap();
    assert!(evidence.contains("covered 0 · lines 13"), "{evidence}");
    assert!(
        lines.iter().any(|line| line.contains("tests.coverage")
            && line.contains("note: branches: unavailable (go)")),
        "{}",
        outcome.stdout
    );
}
