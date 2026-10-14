// ABOUTME: `tests.weakening` end to end: per language, a temporary git history with one commit per kind of
// ABOUTME: weakening, checked with --base/--head and --changed by the real binary (ADR-056, ADR-057).
use std::fs;
use std::path::Path;
use std::process::Command;

use cupel_core::{ActionKind, CheckStatus, Envelope, Severity, SkipReason};
use cupel_fixtures::evidence;
use cupel_fixtures::matrix::Registries;
use cupel_fixtures::run_cupel;
use serde_json::Value;

const CHECK: &str = "tests.weakening";

/// One language's test file, the five weakenings applied to it in order, and a strengthening.
struct Suite {
    lang: &'static str,
    test: &'static str,
    source: &'static str,
    snapshot: &'static str,
    /// A source file the snapshot renders, as the fifth commit changes it from and to: a
    /// snapshot is rewritten together with the code it records (ADR-057).
    code: (&'static str, &'static str, &'static str),
    /// `(from, to)` replacements for: an assertion removed, a matcher loosened, a test skipped,
    /// a test deleted. The fifth weakening rewrites `snapshot`.
    edits: [(&'static str, &'static str); 4],
    /// The names of the tests each weakening touches, in the same order (the fifth is the
    /// snapshot test).
    names: [&'static str; 5],
    /// Appended after the five weakenings: a new, strong test.
    added: &'static str,
}

const RUST: Suite = Suite {
    lang: "rust",
    test: "tests/weak.rs",
    source: r#"use insta::assert_snapshot;

#[test]
fn keeps_both_checks() {
    assert_eq!(1 + 1, 2);
    assert_eq!(2 + 2, 4);
}

#[test]
fn pins_the_value() {
    assert_eq!(3 * 3, 9);
}

#[test]
fn runs_every_time() {
    assert_eq!(4 - 1, 3);
}

#[test]
fn goes_away() {
    assert_eq!(5 + 5, 10);
}

#[test]
fn renders() {
    assert_snapshot!(format!("{}", 6));
}
"#,
    snapshot: "tests/snapshots/weak__renders.snap",
    code: (
        "src/lib.rs",
        "pub fn six() -> u32 {\n    6\n}\n",
        "pub fn six() -> u32 {\n    7\n}\n",
    ),
    edits: [
        ("    assert_eq!(2 + 2, 4);\n", ""),
        ("assert_eq!(3 * 3, 9);", "assert!(3 * 3 > 0);"),
        (
            "#[test]\nfn runs_every_time",
            "#[test]\n#[ignore]\nfn runs_every_time",
        ),
        (
            "#[test]\nfn goes_away() {\n    assert_eq!(5 + 5, 10);\n}\n\n",
            "",
        ),
    ],
    names: [
        "keeps_both_checks",
        "pins_the_value",
        "runs_every_time",
        "goes_away",
        "renders",
    ],
    added: "\n#[test]\nfn adds_a_check() {\n    assert_eq!(7, 7);\n}\n",
};

const GO: Suite = Suite {
    lang: "go",
    test: "weak_test.go",
    source: r#"package weak

import (
	"testing"

	"github.com/gkampitakis/go-snaps/snaps"
	"github.com/stretchr/testify/assert"
)

func TestKeepsBothChecks(t *testing.T) {
	assert.Equal(t, 2, 1+1)
	assert.Equal(t, 4, 2+2)
}

func TestPinsTheValue(t *testing.T) {
	assert.Equal(t, 9, 3*3)
}

func TestRunsEveryTime(t *testing.T) {
	assert.Equal(t, 3, 4-1)
}

func TestGoesAway(t *testing.T) {
	assert.Equal(t, 10, 5+5)
}

func TestRenders(t *testing.T) {
	snaps.MatchSnapshot(t, 6)
}
"#,
    snapshot: "__snapshots__/weak_test.snap",
    code: (
        "weak.go",
        "package weak\n\nfunc Six() int {\n\treturn 6\n}\n",
        "package weak\n\nfunc Six() int {\n\treturn 7\n}\n",
    ),
    edits: [
        ("\tassert.Equal(t, 4, 2+2)\n", ""),
        ("assert.Equal(t, 9, 3*3)", "assert.True(t, 3*3 > 0)"),
        (
            "func TestRunsEveryTime(t *testing.T) {\n",
            "func TestRunsEveryTime(t *testing.T) {\n\tt.Skip(\"flaky\")\n",
        ),
        (
            "func TestGoesAway(t *testing.T) {\n\tassert.Equal(t, 10, 5+5)\n}\n\n",
            "",
        ),
    ],
    names: [
        "TestKeepsBothChecks",
        "TestPinsTheValue",
        "TestRunsEveryTime",
        "TestGoesAway",
        "TestRenders",
    ],
    added: "\nfunc TestAddsACheck(t *testing.T) {\n\tassert.Equal(t, 7, 7)\n}\n",
};

const PYTHON: Suite = Suite {
    lang: "python",
    test: "tests/test_weak.py",
    source: r"import pytest


def test_keeps_both_checks():
    assert 1 + 1 == 2
    assert 2 + 2 == 4


def test_pins_the_value():
    assert 3 * 3 == 9


def test_runs_every_time():
    assert 4 - 1 == 3


def test_goes_away():
    assert 5 + 5 == 10


def test_renders(snapshot):
    assert str(6) == snapshot
",
    snapshot: "tests/__snapshots__/test_weak.ambr",
    code: (
        "src/weak/__init__.py",
        "def six():\n    return 6\n",
        "def six():\n    return 7\n",
    ),
    edits: [
        ("    assert 2 + 2 == 4\n", ""),
        ("assert 3 * 3 == 9", "assert 3 * 3"),
        (
            "def test_runs_every_time",
            "@pytest.mark.skip(reason=\"flaky\")\ndef test_runs_every_time",
        ),
        ("def test_goes_away():\n    assert 5 + 5 == 10\n\n\n", ""),
    ],
    names: [
        "test_keeps_both_checks",
        "test_pins_the_value",
        "test_runs_every_time",
        "test_goes_away",
        "test_renders",
    ],
    added: "\n\ndef test_adds_a_check():\n    assert 7 == 7\n",
};

/// The JavaScript and TypeScript suites share their source.
const ECMA: &str = r#"import { describe, expect, it } from "vitest";

describe("weak", () => {
  it("keeps both checks", () => {
    expect(1 + 1).toBe(2);
    expect(2 + 2).toBe(4);
  });

  it("pins the value", () => {
    expect(3 * 3).toBe(9);
  });

  it("runs every time", () => {
    expect(4 - 1).toBe(3);
  });

  it("goes away", () => {
    expect(5 + 5).toBe(10);
  });

  it("renders", () => {
    expect(String(6)).toMatchSnapshot();
  });
});
"#;

const ECMA_EDITS: [(&str, &str); 4] = [
    ("    expect(2 + 2).toBe(4);\n", ""),
    ("expect(3 * 3).toBe(9);", "expect(3 * 3).toBeTruthy();"),
    ("it(\"runs every time\"", "it.skip(\"runs every time\""),
    (
        "  it(\"goes away\", () => {\n    expect(5 + 5).toBe(10);\n  });\n\n",
        "",
    ),
];

const ECMA_NAMES: [&str; 5] = [
    "keeps both checks",
    "pins the value",
    "runs every time",
    "goes away",
    "renders",
];

const ECMA_ADDED: &str = "\nit(\"adds a check\", () => {\n  expect(7).toBe(7);\n});\n";

const JAVASCRIPT: Suite = Suite {
    lang: "javascript",
    test: "test/weak.test.js",
    source: ECMA,
    snapshot: "test/__snapshots__/weak.test.js.snap",
    code: (
        "src/weak.js",
        "export function six() {\n  return 6;\n}\n",
        "export function six() {\n  return 7;\n}\n",
    ),
    edits: ECMA_EDITS,
    names: ECMA_NAMES,
    added: ECMA_ADDED,
};

const TYPESCRIPT: Suite = Suite {
    lang: "typescript",
    test: "test/weak.test.ts",
    source: ECMA,
    snapshot: "test/__snapshots__/weak.test.ts.snap",
    code: (
        "src/weak.ts",
        "export function six(): number {\n  return 6;\n}\n",
        "export function six(): number {\n  return 7;\n}\n",
    ),
    edits: ECMA_EDITS,
    names: ECMA_NAMES,
    added: ECMA_ADDED,
};

const SUITES: [Suite; 5] = [GO, JAVASCRIPT, PYTHON, RUST, TYPESCRIPT];

/// The kinds, in the order the history introduces them.
const KINDS: [&str; 5] = [
    "assertion.removed",
    "matcher.loosened",
    "test.skipped",
    "test.deleted",
    "snapshot.rewritten",
];

// ---------------------------------------------------------------- git histories

/// A temporary git repository whose default branch is `main`.
struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    fn new() -> Self {
        let repo = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        repo.git(&["init", "--quiet", "--initial-branch", "main"]);
        repo
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, file: &str, text: &str) {
        let path = self.path().join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn read(&self, file: &str) -> String {
        fs::read_to_string(self.path().join(file)).unwrap()
    }

    /// Replaces the one occurrence of `from` in `file` with `to`.
    fn edit(&self, file: &str, from: &str, to: &str) {
        let text = self.read(file);
        assert_eq!(text.matches(from).count(), 1, "{from:?} in {file}");
        self.write(file, &text.replacen(from, to, 1));
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn commit(&self, message: &str) -> String {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    fn check(&self, scope: &[&str]) -> Envelope {
        let mut args = vec!["check", "--only", CHECK, "--format", "json"];
        args.extend(scope);
        let outcome = run_cupel(&args, self.path());
        assert_eq!(outcome.code, Some(0), "{}", outcome.stderr);
        outcome.envelope().clone()
    }
}

/// `suite`'s history: `main` holds the suite and its snapshot; branch `work` adds one commit
/// per weakening, then one that adds a strong test. Returns the repository and the seven SHAs.
fn history(suite: &Suite) -> (Repo, Vec<String>) {
    let repo = Repo::new();
    repo.write(suite.test, suite.source);
    repo.write(suite.snapshot, "6\n");
    repo.write(suite.code.0, suite.code.1);
    let mut shas = vec![repo.commit("the suite")];
    repo.git(&["checkout", "--quiet", "-b", "work"]);
    for ((from, to), kind) in suite.edits.iter().zip(KINDS) {
        repo.edit(suite.test, from, to);
        shas.push(repo.commit(kind));
    }
    repo.write(suite.snapshot, "7\n");
    repo.write(suite.code.0, suite.code.2);
    shas.push(repo.commit("snapshot.rewritten"));
    let text = repo.read(suite.test);
    let added = if suite.source.trim_end().ends_with("});") {
        // ECMA suites: the new test goes inside the describe block.
        let at = text.rfind("});").unwrap();
        format!("{}{}{}", &text[..at], suite.added, &text[at..])
    } else {
        format!("{text}{}", suite.added)
    };
    repo.write(suite.test, &added);
    shas.push(repo.commit("a strong test"));
    (repo, shas)
}

// ---------------------------------------------------------------- assertions

/// (kind, file, the test or snapshot named in the evidence) of every finding, after checking
/// what every weakening finding has in common.
fn findings(envelope: &Envelope) -> Vec<(String, String, String)> {
    let registries = Registries::builtin();
    let manifest = registries
        .checks
        .get(&CHECK.into())
        .expect("tests.weakening is registered")
        .manifest();
    envelope
        .findings
        .iter()
        .map(|finding| {
            assert_eq!(finding.check.as_str(), CHECK);
            assert_eq!(finding.severity, Severity::Fail);
            assert_eq!(finding.action.kind, ActionKind::RestoreTest);
            assert!(finding.repro.contains("--base "), "{}", finding.repro);
            let spec = manifest
                .findings
                .iter()
                .find(|spec| spec.kind == finding.kind)
                .expect("a declared kind");
            let evidence = Value::Object(finding.evidence.0.clone());
            evidence::validate(&spec.evidence, &evidence)
                .unwrap_or_else(|problem| panic!("{}: {problem}", finding.kind));
            let subject = evidence
                .get("test")
                .or_else(|| evidence.get("snapshot"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let file = finding.location.as_ref().unwrap().file.clone();
            (finding.kind.clone(), file, subject)
        })
        .collect()
}

fn status(envelope: &Envelope) -> CheckStatus {
    envelope
        .checks
        .iter()
        .find(|result| result.id.as_str() == CHECK)
        .map_or(CheckStatus::Pass, |result| result.status)
}

// ---------------------------------------------------------------- tests

#[test]
fn each_commit_is_reported_as_exactly_its_weakening() {
    for suite in &SUITES {
        let (repo, shas) = history(suite);
        for (step, kind) in KINDS.iter().enumerate() {
            let (base, head) = (&shas[step], &shas[step + 1]);
            repo.git(&["checkout", "--quiet", "--detach", head]);
            let envelope = repo.check(&["--base", base, "--head", head]);
            let expected = if *kind == "snapshot.rewritten" {
                // The test file did not change, so no changed test claims the snapshot.
                (kind.to_string(), suite.snapshot.to_owned(), suite.snapshot)
            } else {
                (kind.to_string(), suite.test.to_owned(), suite.names[step])
            };
            assert_eq!(
                findings(&envelope),
                [(expected.0, expected.1, expected.2.to_owned())],
                "{} at {kind}",
                suite.lang
            );
            assert_eq!(status(&envelope), CheckStatus::Fail, "{}", suite.lang);
        }
    }
}

#[test]
fn changed_reports_every_weakening_on_the_branch_and_ties_the_snapshot_to_its_test() {
    for suite in &SUITES {
        let (repo, _) = history(suite);
        let envelope = repo.check(&["--changed"]);
        let mut expected: Vec<(String, String, String)> = KINDS
            .iter()
            .zip(suite.names)
            .map(|(kind, name)| {
                let subject = if *kind == "snapshot.rewritten" {
                    suite.snapshot
                } else {
                    name
                };
                (kind.to_string(), suite.test.to_owned(), subject.to_owned())
            })
            .collect();
        let mut actual = findings(&envelope);
        expected.sort();
        actual.sort();
        assert_eq!(actual, expected, "{}", suite.lang);
        let snapshot = envelope
            .findings
            .iter()
            .find(|finding| finding.kind == "snapshot.rewritten")
            .unwrap();
        assert_eq!(
            snapshot.evidence.0["tests"],
            serde_json::json!([suite.names[4]]),
            "{}",
            suite.lang
        );
    }
}

#[test]
fn adding_a_strong_test_is_not_a_weakening() {
    for suite in &SUITES {
        let (repo, shas) = history(suite);
        let envelope = repo.check(&["--base", &shas[5], "--head", &shas[6]]);
        assert_eq!(findings(&envelope), [], "{}", suite.lang);
        assert_eq!(status(&envelope), CheckStatus::Pass, "{}", suite.lang);
        let score = envelope.checks[0].score.as_ref().unwrap();
        assert_eq!((score.value, score.unit.as_str()), (0.0, "weakened"));
    }
}

#[test]
fn a_tree_scope_is_skipped_because_it_needs_a_range() {
    let (repo, _) = history(&RUST);
    let envelope = repo.check(&[]);
    assert_eq!(envelope.findings, []);
    let skipped = envelope.checks[0].skipped.as_ref().expect("skipped");
    assert_eq!(skipped.reason, SkipReason::NeedsRange);
    assert_eq!(
        skipped.lang.as_ref().map(cupel_core::LangId::as_str),
        Some("rust")
    );
    let text = run_cupel(&["check", "--only", CHECK, "--format", "text"], repo.path());
    assert!(
        text.stdout.contains(
            "needs-range (lang rust, see `cupel check --changed --only tests.weakening`)"
        ),
        "{}",
        text.stdout
    );
}

#[test]
fn a_deleted_test_file_is_reported_and_a_moved_test_is_not() {
    let repo = Repo::new();
    repo.write(
        "tests/a.rs",
        "#[test]\nfn moves() {\n    assert_eq!(1, 1);\n}\n",
    );
    repo.write(
        "tests/b.rs",
        "#[test]\nfn stays_put() {\n    assert_eq!(2, 2);\n}\n",
    );
    let base = repo.commit("two files");
    fs::remove_file(repo.path().join("tests/a.rs")).unwrap();
    fs::remove_file(repo.path().join("tests/b.rs")).unwrap();
    repo.write(
        "tests/c.rs",
        "#[test]\nfn moves() {\n    assert_eq!(1, 1);\n}\n",
    );
    repo.commit("move one, delete the other");

    let envelope = repo.check(&["--base", &base]);
    assert_eq!(
        findings(&envelope),
        [(
            "test.deleted".to_owned(),
            "tests/b.rs".to_owned(),
            "stays_put".to_owned()
        )]
    );
    let deleted = &envelope.findings[0];
    assert_eq!(deleted.location.as_ref().unwrap().line, 2);
    assert_eq!(deleted.evidence.0["file"], "tests/b.rs");
}

#[test]
fn a_snapshot_file_that_grew_with_a_new_test_is_not_a_rewrite() {
    let repo = Repo::new();
    repo.write(JAVASCRIPT.test, ECMA);
    repo.write(JAVASCRIPT.snapshot, "6\n");
    let base = repo.commit("the suite");
    let grown = ECMA.replace(
        "\n});\n",
        "\n  it(\"renders twice\", () => {\n    expect(\"66\").toMatchSnapshot();\n  });\n});\n",
    );
    assert_ne!(grown, ECMA);
    repo.write(JAVASCRIPT.test, &grown);
    repo.write(JAVASCRIPT.snapshot, "6\n66\n");
    repo.commit("a new snapshot test");

    let envelope = repo.check(&["--base", &base]);
    assert_eq!(findings(&envelope), [], "{:?}", envelope.findings);
}

#[test]
fn a_dry_run_plans_the_check_without_checking_out_the_base() {
    let (repo, _) = history(&RUST);
    let outcome = run_cupel(
        &[
            "check",
            "--only",
            CHECK,
            "--changed",
            "--dry-run",
            "--format",
            "json",
        ],
        repo.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stderr);
    let plan = outcome.envelope().plan.clone().expect("a plan");
    assert!(
        plan.would_run
            .iter()
            .any(|check| check.id.as_str() == CHECK),
        "{plan:?}"
    );
    assert!(!repo.path().join(".cupel/work").exists());
}
