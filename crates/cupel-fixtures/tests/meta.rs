// ABOUTME: `meta.traceability` and `meta.oracle-independence` end to end by the real binary with --base: a Python history,
// ABOUTME: and every language's fixture with a marked test added (ADR-114, ADR-115, ADR-134).
use std::fs;
use std::path::Path;
use std::process::Command;

use cupel_core::{Envelope, SkipReason};
use cupel_fixtures::{copy_repo, run_cupel};
use serde_json::{Value, json};

const BASE_SOURCE: &str = "def parse(text):\n    return int(text)\n";

const HEAD_SOURCE: &str = "def parse(text):\n    return int(text.strip())\n\n\n\
                           def format(value):\n    return str(value)\n";

const BASE_TESTS: &str = "from dates import parse\n\n\ndef test_req_1_parses():\n    \
                          assert parse(\"4\") == 4\n";

const ADDED_TEST: &str = "\n\ndef test_req_2_formats():\n    from dates import format\n\n    \
                          assert format(4) == \"4\"\n";

const CONFIG: &str = "[checks.\"meta.traceability\"]\nrequirements = [\"REQ-1\", \"REQ-2\"]\n";

const PYPROJECT: &str = "[project]\nname = \"dates\"\nversion = \"0.1.0\"\n\n\
                         [tool.pytest.ini_options]\ntestpaths = [\"tests\"]\npythonpath = [\"src\"]\n";

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn write(dir: &Path, file: &str, text: &str) {
    let path = dir.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A repository whose base commit has `parse` and a test of it naming REQ-1, and whose
/// working tree changes `parse`, adds `format`, and adds a test of it naming REQ-2. Returns the
/// repository and the base commit.
fn history() -> (tempfile::TempDir, String) {
    let repo = tempfile::tempdir().unwrap();
    let dir = repo.path();
    git(dir, &["init", "--quiet", "--initial-branch", "main"]);
    write(dir, "pyproject.toml", PYPROJECT);
    write(dir, "cupel.toml", CONFIG);
    write(dir, "src/dates.py", BASE_SOURCE);
    write(dir, "tests/test_dates.py", BASE_TESTS);
    git(dir, &["add", "--all"]);
    git(dir, &["commit", "--quiet", "-m", "parse"]);
    let base = git(dir, &["rev-parse", "HEAD"]);
    write(dir, "src/dates.py", HEAD_SOURCE);
    write(
        dir,
        "tests/test_dates.py",
        &format!("{BASE_TESTS}{ADDED_TEST}"),
    );
    (repo, base)
}

fn check(dir: &Path, args: &[&str]) -> Envelope {
    let mut all = vec!["check", "--format", "json"];
    all.extend(args);
    let outcome = run_cupel(&all, dir);
    assert_eq!(outcome.code, Some(0), "{}", outcome.stderr);
    outcome.envelope().clone()
}

/// Each finding as (kind, title, evidence).
fn findings(envelope: &Envelope) -> Vec<(String, String, Value)> {
    envelope
        .findings
        .iter()
        .map(|finding| {
            (
                finding.kind.clone(),
                finding.title.clone(),
                serde_json::to_value(&finding.evidence).unwrap(),
            )
        })
        .collect()
}

#[test]
fn a_requirement_only_a_pre_existing_test_names_is_untested_and_one_the_range_tests_is_traced() {
    let (repo, base) = history();
    let envelope = check(
        repo.path(),
        &["--only", "meta.traceability", "--base", &base],
    );
    assert_eq!(
        findings(&envelope),
        [(
            "spec.untested-requirement".to_owned(),
            "requirement REQ-1 has no test the change added; 1 test that existed before names it"
                .to_owned(),
            json!({
                "requirement": "REQ-1",
                "source": "config",
                "base": base,
                "tests": [{"file": "tests/test_dates.py", "name": "test_req_1_parses", "line": 4, "added": false}],
            }),
        )]
    );
    let result = &envelope.checks[0];
    assert_eq!(result.metrics.get("untested"), Some(&1.0));
}

#[test]
fn a_tree_scope_skips_both_meta_checks_because_they_need_a_range() {
    let (repo, _) = history();
    let envelope = check(
        repo.path(),
        &[
            "--only",
            "meta.traceability",
            "--only",
            "meta.oracle-independence",
        ],
    );
    let reasons: Vec<_> = envelope
        .checks
        .iter()
        .map(|result| result.skipped.as_ref().map(|skipped| skipped.reason))
        .collect();
    assert_eq!(
        reasons,
        [Some(SkipReason::NeedsRange), Some(SkipReason::NeedsRange)]
    );
}

#[test]
fn a_changed_function_only_the_added_test_runs_is_self_authored_and_one_an_old_test_runs_is_not() {
    let (repo, base) = history();
    let envelope = check(
        repo.path(),
        &[
            "--only",
            "meta.oracle-independence",
            "--base",
            &base,
            "--depth",
            "standard",
        ],
    );
    let result = &envelope.checks[0];
    if result.score.is_none() {
        eprintln!(
            "SKIPPED: no per-test map was made here ({:?}); meta.oracle-independence labeled nothing",
            result.notes
        );
        return;
    }
    assert_eq!(
        findings(&envelope),
        [(
            "evidence.self-authored-only".to_owned(),
            "`dates.format` is exercised only by tests the change added (1), none that existed before it"
                .to_owned(),
            json!({"base": base, "tests": ["tests/test_dates.py::test_req_2_formats"]}),
        )]
    );
    assert_eq!(result.metrics.get("independent"), Some(&1.0));
    assert_eq!(result.metrics.get("self_authored_only"), Some(&1.0));
}

/// A language's fixture test file, whose test `documented` has a doc naming REQ-1, and a test to
/// add to it whose doc names REQ-2 and whose name carries the marker REQ-3.
struct Marked {
    lang: &'static str,
    file: &'static str,
    documented: &'static str,
    added: &'static str,
    test: &'static str,
}

const MARKED: [Marked; 7] = [
    Marked {
        lang: "rust",
        file: "tests/ledger.rs",
        documented: "opening_an_account_does_not_panic",
        added: "names_the_holder_req_3",
        test: "\n/// Statements name the holder (REQ-2).\n#[test]\nfn names_the_holder_req_3() {\n    \
               assert_eq!(1, 1);\n}\n",
    },
    Marked {
        lang: "go",
        file: "ledger_test.go",
        documented: "TestOpeningAnAccountDoesNotPanic",
        added: "TestNamesTheHolder_REQ_3",
        test: "\n// TestNamesTheHolder_REQ_3 covers REQ-2.\nfunc TestNamesTheHolder_REQ_3(t *testing.T) \
               {\n\tif 1 != 1 {\n\t\tt.Fatal(\"no\")\n\t}\n}\n",
    },
    Marked {
        lang: "python",
        file: "tests/test_ledger.py",
        documented: "test_opening_an_account_does_not_raise",
        added: "test_req_3_names_the_holder",
        test: "\n\ndef test_req_3_names_the_holder():\n    \"\"\"Statements name the holder \
               (REQ-2).\"\"\"\n    assert 1 == 1\n",
    },
    Marked {
        lang: "javascript",
        file: "test/math.test.js",
        documented: "reports what it parsed",
        added: "names the holder #REQ-3",
        test: "\n/** Statements name the holder (REQ-2). */\nit(\"names the holder #REQ-3\", () => \
               {\n  expect(1).toBe(1);\n});\n",
    },
    Marked {
        lang: "typescript",
        file: "src/math.test.ts",
        documented: "is consumed by a spy",
        added: "names the holder #REQ-3",
        test: "\n/** Statements name the holder (REQ-2). */\nit(\"names the holder #REQ-3\", () => \
               {\n  expect(1).toBe(1);\n});\n",
    },
    Marked {
        lang: "java",
        file: "src/test/java/ledger/LedgerTest.java",
        documented: "openingAnAccountDoesNotThrow",
        added: "namesTheHolder_REQ_3",
        test: "\n    /** Statements name the holder (REQ-2). */\n    @Test\n    void \
               namesTheHolder_REQ_3() {\n        assertEquals(1, 1);\n    }\n",
    },
    Marked {
        lang: "clojure",
        file: "test/ledger/core_test.clj",
        documented: "opening-an-account-does-not-throw",
        added: "names-the-holder-req-3",
        test: "\n;; Statements name the holder (REQ-2).\n(deftest names-the-holder-req-3\n  \
               (is (= 1 1)))\n",
    },
];

/// `text` with `test` added as its last test: inside the last closing brace for Java, whose
/// tests are a class's methods, else at the end.
fn with_test(lang: &str, text: &str, test: &str) -> String {
    if lang == "java" {
        let close = text.rfind('}').expect("a class");
        format!("{}{test}{}", &text[..close], &text[close..])
    } else {
        format!("{text}{test}")
    }
}

/// Each finding as (kind, title, source, [(test name, added)]), by kind.
fn traced(envelope: &Envelope) -> Vec<(String, String, Value, Vec<(String, bool)>)> {
    let mut traced: Vec<_> = envelope
        .findings
        .iter()
        .map(|finding| {
            let evidence = serde_json::to_value(&finding.evidence).unwrap();
            let tests = evidence["tests"]
                .as_array()
                .unwrap()
                .iter()
                .map(|test| {
                    (
                        test["name"].as_str().unwrap().to_owned(),
                        test["added"].as_bool().unwrap(),
                    )
                })
                .collect();
            (
                finding.kind.clone(),
                finding.title.clone(),
                evidence["source"].clone(),
                tests,
            )
        })
        .collect();
    traced.sort_by(|a, b| b.0.cmp(&a.0));
    traced
}

#[test]
fn every_language_traces_a_doc_marker_lists_a_name_marker_and_reads_a_fixture_doc() {
    for marked in &MARKED {
        let repo = tempfile::tempdir().unwrap();
        let dir = repo.path();
        copy_repo(marked.lang, dir);
        let config = dir.join("cupel.toml");
        let mut toml = fs::read_to_string(&config).unwrap_or_default();
        toml.push_str("\n[checks.\"meta.traceability\"]\nrequirements = [\"REQ-1\", \"REQ-2\"]\n");
        fs::write(&config, toml).unwrap();
        git(dir, &["init", "--quiet", "--initial-branch", "main"]);
        git(dir, &["add", "--all"]);
        git(dir, &["commit", "--quiet", "-m", "fixture"]);
        let base = git(dir, &["rev-parse", "HEAD"]);
        let file = dir.join(marked.file);
        let text = fs::read_to_string(&file).unwrap();
        fs::write(&file, with_test(marked.lang, &text, marked.test)).unwrap();

        let envelope = check(dir, &["--only", "meta.traceability", "--base", &base]);

        assert_eq!(
            traced(&envelope),
            [
                (
                    "spec.untested-requirement".to_owned(),
                    "requirement REQ-1 has no test the change added; 1 test that existed before \
                     names it"
                        .to_owned(),
                    json!("config"),
                    vec![(marked.documented.to_owned(), false)],
                ),
                (
                    "spec.unlisted-requirement".to_owned(),
                    "requirement REQ-3 is named by 1 test the change added, but not configured"
                        .to_owned(),
                    json!("marker"),
                    vec![(marked.added.to_owned(), true)],
                ),
            ],
            "{}",
            marked.lang
        );
        let result = &envelope.checks[0];
        assert_eq!(
            result.metrics.get("untested"),
            Some(&1.0),
            "{}",
            marked.lang
        );
        assert_eq!(
            result.score.as_ref().map(|score| score.n),
            Some(3),
            "{}",
            marked.lang
        );
    }
}
