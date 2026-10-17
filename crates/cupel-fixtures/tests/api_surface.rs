// ABOUTME: `static.api-surface` end to end: per language, a temporary git history that removes, changes, and adds
// ABOUTME: public items, checked with --base and --changed by the real binary, and the `no-breaking` gate (ADR-101).
use std::fs;
use std::path::Path;
use std::process::Command;

use cupel_core::{CheckStatus, Envelope, Severity, SkipReason};
use cupel_fixtures::evidence;
use cupel_fixtures::matrix::Registries;
use cupel_fixtures::run_cupel;
use serde_json::Value;

const CHECK: &str = "static.api-surface";

/// One language's source file at the base commit and at head: head keeps `keep`, removes
/// `gone`, gives `grows` a second parameter, and adds `fresh`.
struct Library {
    lang: &'static str,
    file: &'static str,
    base: &'static str,
    head: &'static str,
}

const LIBRARIES: [Library; 5] = [
    Library {
        lang: "go",
        file: "ledger.go",
        base: "package ledger\n\nfunc Keep(x int) int {\n\treturn x\n}\n\nfunc Gone() int {\n\treturn 0\n}\n\nfunc Grows(x int) int {\n\treturn x\n}\n",
        head: "package ledger\n\nfunc Keep(x int) int {\n\treturn x\n}\n\nfunc Grows(x int, y int) int {\n\treturn x + y\n}\n\nfunc Fresh() int {\n\treturn 1\n}\n",
    },
    Library {
        lang: "javascript",
        file: "ledger.js",
        base: "export function keep(x) {\n  return x;\n}\n\nexport function gone() {\n  return 0;\n}\n\nexport function grows(x) {\n  return x;\n}\n",
        head: "export function keep(x) {\n  return x;\n}\n\nexport function grows(x, y) {\n  return x + y;\n}\n\nexport function fresh() {\n  return 1;\n}\n",
    },
    Library {
        lang: "python",
        file: "ledger.py",
        base: "def keep(x: int) -> int:\n    return x\n\n\ndef gone() -> int:\n    return 0\n\n\ndef grows(x: int) -> int:\n    return x\n",
        head: "def keep(x: int) -> int:\n    return x\n\n\ndef grows(x: int, y: int) -> int:\n    return x + y\n\n\ndef fresh() -> int:\n    return 1\n",
    },
    Library {
        lang: "rust",
        file: "src/lib.rs",
        base: "pub fn keep(x: i64) -> i64 {\n    x\n}\n\npub fn gone() -> i64 {\n    0\n}\n\npub fn grows(x: i64) -> i64 {\n    x\n}\n",
        head: "pub fn keep(x: i64) -> i64 {\n    x\n}\n\npub fn grows(x: i64, y: i64) -> i64 {\n    x + y\n}\n\npub fn fresh() -> i64 {\n    1\n}\n",
    },
    Library {
        lang: "typescript",
        file: "ledger.ts",
        base: "export function keep(x: number): number {\n  return x;\n}\n\nexport function gone(): number {\n  return 0;\n}\n\nexport function grows(x: number): number {\n  return x;\n}\n",
        head: "export function keep(x: number): number {\n  return x;\n}\n\nexport function grows(x: number, y: number): number {\n  return x + y;\n}\n\nexport function fresh(): number {\n  return 1;\n}\n",
    },
];

/// What every language reports: (kind, change, the item's name).
const EXPECTED: [(&str, &str, &str); 3] = [
    ("api.breaking", "removed", "gone"),
    ("api.breaking", "changed", "grows"),
    ("api.widened", "added", "fresh"),
];

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

    /// `check --only static.api-surface` with `scope`, which must exit `code`.
    fn check(&self, scope: &[&str], code: i32) -> Envelope {
        let mut args = vec!["check", "--only", CHECK, "--format", "json"];
        args.extend(scope);
        let outcome = run_cupel(&args, self.path());
        assert_eq!(outcome.code, Some(code), "{}", outcome.stderr);
        outcome.envelope().clone()
    }
}

/// `library`'s history: `main` holds its base, branch `work` its head. Returns the repository
/// and the base commit.
fn history(library: &Library) -> (Repo, String) {
    let repo = Repo::new();
    repo.write(library.file, library.base);
    let base = repo.commit("the library");
    repo.git(&["checkout", "--quiet", "-b", "work"]);
    repo.write(library.file, library.head);
    repo.commit("the change");
    (repo, base)
}

/// The last segment of an item's path (`ledger.Gone`, `gone`), lowercased.
fn name(path: &str) -> String {
    path.rsplit(['.', ':'])
        .next()
        .unwrap_or(path)
        .to_lowercase()
}

/// (kind, change, name) of every finding, sorted (the envelope ranks findings), after checking
/// their evidence against the manifest.
fn findings(envelope: &Envelope) -> Vec<(String, String, String)> {
    let registries = Registries::builtin();
    let check = registries
        .checks
        .get(&cupel_core::CheckId::from(CHECK))
        .unwrap();
    envelope
        .findings
        .iter()
        .map(|finding| {
            let spec = check
                .manifest()
                .findings
                .iter()
                .find(|spec| spec.kind == finding.kind)
                .expect("a declared kind");
            let fields = Value::Object(finding.evidence.0.clone());
            evidence::validate(&spec.evidence, &fields).unwrap();
            (
                finding.kind.clone(),
                fields["change"].as_str().unwrap().to_owned(),
                name(fields["path"].as_str().unwrap()),
            )
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn expected() -> Vec<(String, String, String)> {
    let mut expected: Vec<_> = EXPECTED
        .iter()
        .map(|(kind, change, name)| ((*kind).to_owned(), (*change).to_owned(), (*name).to_owned()))
        .collect();
    expected.sort();
    expected
}

/// The finding whose evidence says `change`.
fn with_change<'a>(envelope: &'a Envelope, change: &str) -> &'a cupel_core::Finding {
    envelope
        .findings
        .iter()
        .find(|finding| finding.evidence.0["change"] == change)
        .expect("a finding with the change")
}

#[test]
fn base_reports_the_removed_changed_and_added_items_in_every_language() {
    for library in &LIBRARIES {
        let (repo, base) = history(library);
        let envelope = repo.check(&["--base", &base], 0);
        assert_eq!(findings(&envelope), expected(), "{}", library.lang);
        let result = &envelope.checks[0];
        assert_eq!(result.status, CheckStatus::Fail, "{}", library.lang);
        assert_eq!(
            result.metrics.get("breaking"),
            Some(&2.0),
            "{}",
            library.lang
        );
        assert_eq!(
            result.metrics.get("widened"),
            Some(&1.0),
            "{}",
            library.lang
        );
        let removed = with_change(&envelope, "removed");
        assert_eq!(removed.severity, Severity::Warn);
        assert_eq!(
            removed
                .location
                .as_ref()
                .map(|at| (at.file.as_str(), at.line)),
            Some((library.file, line_of(&library.base.to_lowercase(), "gone("))),
            "{}: the removed item is located where it was at the base",
            library.lang
        );
        assert_eq!(
            removed.repro,
            format!("cupel check --only {CHECK} --base {base}")
        );
        assert_eq!(with_change(&envelope, "added").severity, Severity::Info);
    }
}

/// The 1-based line of the first line of `text` holding `needle`.
fn line_of(text: &str, needle: &str) -> u32 {
    let index = text
        .lines()
        .position(|line| line.contains(needle))
        .expect("the needle is in the text");
    u32::try_from(index).unwrap() + 1
}

#[test]
fn changed_reports_the_same_against_the_merge_base_with_main() {
    for library in &LIBRARIES {
        let (repo, _) = history(library);
        let envelope = repo.check(&["--changed"], 0);
        assert_eq!(findings(&envelope), expected(), "{}", library.lang);
    }
}

#[test]
fn an_unchanged_surface_passes_and_a_tree_scope_is_skipped_because_it_needs_a_range() {
    let library = &LIBRARIES[2];
    let repo = Repo::new();
    repo.write(library.file, library.base);
    let base = repo.commit("the library");
    repo.write("notes.py", "def helper() -> int:\n    return 0\n");
    repo.write(
        library.file,
        &library.base.replace("return x", "return x * 1"),
    );
    repo.commit("a body changes");
    let ranged = repo.check(&["--base", &base], 0);
    assert_eq!(
        findings(&ranged),
        [(
            "api.widened".to_owned(),
            "added".to_owned(),
            "helper".to_owned()
        )]
    );
    assert_eq!(ranged.checks[0].status, CheckStatus::Pass);
    let tree = repo.check(&[], 0);
    let skipped = tree.checks[0].skipped.as_ref().expect("skipped");
    assert_eq!(skipped.reason, SkipReason::NeedsRange);
}

#[test]
fn the_no_breaking_gate_fails_on_a_break_and_passes_once_it_is_suppressed() {
    let (repo, base) = history(&LIBRARIES[2]);
    repo.write(
        "cupel.toml",
        "[gate.changed]\n\"static.api-surface\" = \"no-breaking\"\n",
    );
    let gated = run_cupel(
        &[
            "check", "--only", CHECK, "--base", &base, "--format", "json",
        ],
        repo.path(),
    );
    assert_eq!(gated.code, Some(1), "{}", gated.stderr);
    let gate = gated.envelope().summary.gate.clone().expect("gated");
    assert_eq!(gate.rules[0].actual, Some(2.0));
    for finding in &gated.envelope().findings {
        if finding.kind == "api.breaking" {
            let suppressed = run_cupel(
                &[
                    "suppress",
                    finding.id.as_str(),
                    "--reason",
                    "intended",
                    "--format",
                    "json",
                ],
                repo.path(),
            );
            assert_eq!(suppressed.code, Some(0), "{}", suppressed.stderr);
        }
    }
    let passed = run_cupel(
        &[
            "check", "--only", CHECK, "--base", &base, "--format", "json",
        ],
        repo.path(),
    );
    assert_eq!(passed.code, Some(0), "{}", passed.stdout);
}
