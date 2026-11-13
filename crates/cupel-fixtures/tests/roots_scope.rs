// ABOUTME: `[roots]` scope every check end to end (ADR-177): with `packages/shared` the one configured root, the
// ABOUTME: complex function, clone pair, and import cycle of `apps/agent` are neither reported nor counted.
use std::path::Path;
use std::process::Command;

use cupel_core::Envelope;
use cupel_fixtures::{copy_repo, run_cupel, write_file};

const STATIC: [&str; 4] = [
    "static.complexity",
    "static.clones",
    "static.deps",
    "static.dead-code",
];

/// The fixture copied into a fresh git repository with one commit on `main`.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("roots-scope", dir.path());
    for args in [
        &["init", "--quiet", "--initial-branch", "main"][..],
        &["add", "--all"],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "--quiet",
            "-m",
            "first",
        ],
    ] {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    }
    dir
}

/// The static checks run over `dir` with `extra` scope flags, without a gate.
fn check(dir: &Path, extra: &[&str]) -> Envelope {
    let mut args = vec!["check", "--only"];
    args.extend(STATIC);
    args.extend(["--no-gate", "--format", "json"]);
    args.extend(extra);
    let outcome = run_cupel(&args, dir);
    assert_eq!(
        outcome.code,
        Some(0),
        "{}{}",
        outcome.stdout,
        outcome.stderr
    );
    outcome.envelope().clone()
}

/// Each finding's kind and file, in the envelope's order.
fn findings(envelope: &Envelope) -> Vec<(String, String)> {
    envelope
        .findings
        .iter()
        .map(|finding| {
            let file = finding.location.as_ref().map(|at| at.file.clone());
            (finding.kind.clone(), file.unwrap_or_default())
        })
        .collect()
}

/// The metric `name` of check `id`.
fn metric(envelope: &Envelope, id: &str, name: &str) -> Option<f64> {
    let result = envelope
        .checks
        .iter()
        .find(|result| result.id.as_str() == id)?;
    result.metrics.get(name).copied()
}

#[test]
fn files_outside_every_root_produce_no_finding_and_no_metric() {
    let dir = repo();
    let envelope = check(dir.path(), &[]);
    let scope = envelope.run.scope.as_ref().expect("a scope");
    assert_eq!(scope.files, 1, "only packages/shared/src/index.ts");
    assert_eq!(scope.roots, ["packages/shared"]);
    assert_eq!(findings(&envelope), Vec::<(String, String)>::new());
    let cyclomatic = metric(&envelope, "static.complexity", "cyclomatic_max");
    assert_eq!(cyclomatic, Some(1.0), "`add`, not apps/agent's `classify`");
    assert_eq!(metric(&envelope, "static.clones", "pairs"), Some(0.0));
    assert_eq!(metric(&envelope, "static.deps", "cycles"), Some(0.0));
}

#[test]
fn without_roots_the_whole_repository_is_in_scope() {
    let dir = repo();
    std::fs::remove_file(dir.path().join("cupel.toml")).unwrap();
    let envelope = check(dir.path(), &[]);
    assert_eq!(
        envelope.run.scope.as_ref().map(|scope| scope.files),
        Some(3)
    );
    let agent = |kind: &str, file: &str| (kind.to_owned(), format!("apps/agent/src/{file}"));
    let kinds: Vec<_> = findings(&envelope)
        .into_iter()
        .filter(|(_, file)| file.starts_with("apps/"))
        .collect();
    assert_eq!(
        kinds,
        [
            agent("dep.cycle", "copy.ts"),
            agent("clone.pair", "copy.ts"),
            agent("complexity.high", "tangle.ts"),
        ]
    );
    let cyclomatic = metric(&envelope, "static.complexity", "cyclomatic_max");
    assert_eq!(cyclomatic, Some(15.0), "apps/agent's `classify`");
    assert_eq!(metric(&envelope, "static.clones", "pairs"), Some(1.0));
    assert_eq!(metric(&envelope, "static.deps", "cycles"), Some(1.0));
}

#[test]
fn a_change_outside_every_root_is_not_in_a_changed_scope() {
    let dir = repo();
    let tangle = "apps/agent/src/tangle.ts";
    let source = std::fs::read_to_string(dir.path().join(tangle)).unwrap();
    write_file(dir.path(), tangle, &source.replace("\"ok\"", "\"fine\""));
    let envelope = check(dir.path(), &["--changed"]);
    let scope = envelope.run.scope.as_ref().expect("a scope");
    assert_eq!(scope.files, 0, "{scope:?}");
    assert_eq!(findings(&envelope), Vec::<(String, String)>::new());
}

#[test]
fn paths_outside_every_root_are_refused() {
    let dir = repo();
    let outcome = run_cupel(
        &[
            "check",
            "--only",
            "static.complexity",
            "--paths",
            "apps/agent",
            "--format",
            "json",
        ],
        dir.path(),
    );
    assert_eq!(outcome.code, Some(2), "{}", outcome.stdout);
    let titles: Vec<&str> = outcome
        .envelope()
        .findings
        .iter()
        .map(|finding| finding.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["`--paths apps/agent` names no source file under a configured root (packages/shared)"]
    );
}
