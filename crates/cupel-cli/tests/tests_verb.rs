// ABOUTME: `cupel tests --impacted` end to end (ADR-077, ADR-006): on the per-test fixtures, changing one of two
// ABOUTME: functions impacts exactly one test, by per-test coverage or, for JavaScript, by the import graph.
use serde_json::{Value, json};
use support::Repo;

mod support;

/// A git repository holding `per-test/<lang>`, committed on `main`, with `add` changed in
/// `file` (`a + b` becomes `b + a`); `None` without git.
fn changed(lang: &str, file: &str) -> Option<Repo> {
    let repo = Repo::new();
    cupel_fixtures::copy_repo(&format!("per-test/{lang}"), &repo.root());
    repo.write(".gitignore", "target/\n.cupel/\nCargo.lock\n");
    if !(repo.git(&["init", "--quiet", "--initial-branch", "main"])
        && repo.git(&["add", "."])
        && repo.git(&["commit", "--quiet", "-m", "fixture"]))
    {
        eprintln!("SKIPPED: git not installed");
        return None;
    }
    let path = repo.path(file);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("a + b"), "{file}");
    std::fs::write(&path, text.replace("a + b", "b + a")).unwrap();
    Some(repo)
}

/// The `tests` report of `cupel tests --impacted --changed` in `repo`, which must pass.
fn impacted(repo: &Repo) -> Value {
    let outcome = repo.run(&["tests", "--impacted", "--changed", "--format", "json"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("pass"));
    envelope["tests"].clone()
}

/// Each impacted test as `(id, reason)`.
fn tests_of(report: &Value) -> Vec<(String, String)> {
    report["impacted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|test| {
            (
                test["id"].as_str().unwrap().to_owned(),
                test["reason"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// Changing `add` in `file` of `per-test/<lang>` impacts exactly `test`, chosen by the
/// language's per-test map, with a narrowed command; skipped when the map's tool is missing.
fn assert_one_test_by_coverage(lang: &str, file: &str, test: &str, reason: &str, command: &str) {
    let Some(repo) = changed(lang, file) else {
        return;
    };
    let report = impacted(&repo);
    let basis = &report["languages"][0];
    if basis["per_test_map"] == json!(false) {
        let why = basis["fallback"].as_str().unwrap_or_default();
        assert!(why.contains("not installed"), "{report}");
        eprintln!("SKIPPED: {why}");
        return;
    }
    assert_eq!(report["per_test_map"], json!(true));
    assert_eq!(tests_of(&report), [(test.to_owned(), reason.to_owned())]);
    let commands: Vec<&Value> = report["commands"].as_array().unwrap().iter().collect();
    assert_eq!(commands.len(), 1, "{report}");
    assert_eq!(commands[0]["narrowed"], json!(true));
    assert_eq!(commands[0]["command"], json!(command));
}

#[test]
fn changing_one_rust_function_impacts_only_the_test_that_covers_it() {
    assert_one_test_by_coverage(
        "rust",
        "src/lib.rs",
        "calc tests::adds",
        "covers add (src/lib.rs:5)",
        "cargo nextest run --workspace -E '(binary_id(=calc) & test(=tests::adds))'",
    );
}

#[test]
fn changing_one_go_function_impacts_only_the_test_that_covers_it() {
    assert_one_test_by_coverage(
        "go",
        "calc.go",
        "example.com/calc/TestAdd",
        "covers calc.Add (calc.go:6)",
        "go test -json ./... -run '^(TestAdd)$'",
    );
}

#[test]
fn changing_one_python_function_impacts_only_the_test_that_covers_it() {
    assert_one_test_by_coverage(
        "python",
        "calc.py",
        "tests/test_calc.py::test_add",
        "covers calc.add (calc.py:6)",
        "python -m pytest tests/test_calc.py::test_add",
    );
}

/// A second `tests --impacted` run reads `lang`'s per-test map from the cache the first wrote
/// under `.cupel/cache/per-test/<producer>/`: it measures nothing and impacts the same test.
fn assert_the_second_run_is_cached(lang: &str, file: &str, producer: &str) {
    let Some(repo) = changed(lang, file) else {
        return;
    };
    let first = impacted(&repo);
    if first["languages"][0]["per_test_map"] == json!(false) {
        eprintln!("SKIPPED: {}", first["languages"][0]["fallback"]);
        return;
    }
    let second = impacted(&repo);
    let counts = |report: &Value| {
        let basis = &report["languages"][0];
        (basis["measured"].clone(), basis["reused"].clone())
    };
    assert_eq!(
        (counts(&first), counts(&second)),
        ((json!(2), json!(0)), (json!(0), json!(2)))
    );
    assert_eq!(tests_of(&first), tests_of(&second));
    assert!(
        repo.path(&format!(".cupel/cache/per-test/{producer}"))
            .is_dir()
    );
    let text = repo.run(&["tests", "--impacted", "--changed", "--format", "text"]);
    let line = format!("{lang}  per-test map from {producer} (cached)");
    assert!(text.stdout.contains(&line), "{line}\nin:\n{}", text.stdout);
}

#[test]
fn a_second_python_run_reads_the_per_test_map_from_its_cache() {
    assert_the_second_run_is_cached("python", "calc.py", "tool.pytest-cov-per-test");
}

#[test]
fn a_second_rust_run_reads_the_per_test_map_from_its_cache() {
    assert_the_second_run_is_cached("rust", "src/lib.rs", "tool.llvm-cov-per-test");
}

#[test]
fn javascript_falls_back_to_the_test_files_that_import_the_change_and_says_so() {
    let Some(repo) = changed("javascript", "add.js") else {
        return;
    };
    let report = impacted(&repo);
    assert_eq!(report["per_test_map"], json!(false));
    assert_eq!(
        tests_of(&report),
        [("add.test.js".to_owned(), "imports add.js".to_owned())]
    );
    let basis = &report["languages"][0];
    assert_eq!(
        (&basis["lang"], &basis["per_test_map"], &basis["producer"]),
        (&json!("javascript"), &json!(false), &Value::Null)
    );
    assert!(
        basis["fallback"]
            .as_str()
            .unwrap()
            .contains("no producer makes per-test-map"),
        "{basis}"
    );
    assert_eq!(
        report["commands"],
        json!([{"lang": "javascript", "runner": "tool.jest", "dir": ".",
                "command": "npx jest add.test.js", "narrowed": true}])
    );
}

#[test]
fn a_changed_scope_with_no_changes_impacts_nothing_and_passes() {
    let Some(repo) = changed("javascript", "add.js") else {
        return;
    };
    assert!(repo.git(&["checkout", "--quiet", "--", "add.js"]));
    let report = impacted(&repo);
    assert_eq!(report["impacted"], json!([]));
    assert_eq!(report["commands"], json!([]));
}

#[test]
fn a_paths_scope_impacts_the_tests_of_every_line_of_its_files() {
    let Some(repo) = changed("javascript", "add.js") else {
        return;
    };
    let outcome = repo.run(&[
        "tests",
        "--impacted",
        "--paths",
        "scale.js",
        "--format",
        "json",
    ]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(
        tests_of(&outcome.envelope()["tests"]),
        [("scale.test.js".to_owned(), "imports scale.js".to_owned())]
    );
}

#[test]
fn tests_without_impacted_is_a_usage_error_naming_the_flag() {
    let repo = Repo::new();
    let outcome = repo.run(&["tests", "--format", "json"]);
    assert_eq!(outcome.code, 2, "{}", outcome.stdout);
    assert_eq!(support::usage_argument(&outcome), json!("--impacted"));
}

#[test]
fn the_text_rendering_lists_the_tests_their_reasons_and_the_command() {
    let Some(repo) = changed("javascript", "add.js") else {
        return;
    };
    let outcome = repo.run(&["tests", "--impacted", "--changed", "--format", "text"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    for line in [
        "tests  1 impacted · per-test map: no",
        "  add.test.js  javascript",
        "    imports add.js",
        "  run in .: npx jest add.test.js",
    ] {
        assert!(
            outcome.stdout.contains(line),
            "{line}\nin:\n{}",
            outcome.stdout
        );
    }
}
