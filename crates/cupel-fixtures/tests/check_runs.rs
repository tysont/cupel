// ABOUTME: `cupel check` over the fixture repositories: per-language threshold overrides, scope
// ABOUTME: languages in a mixed tree, and a timeout that cuts the run short (exit 5, partial).
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use cupel_core::{CheckStatus, Envelope, ExitCode, SkipReason, Status};
use cupel_fixtures::{copy_repo, run_cupel};

/// A tree holding the Go and Rust fixture repositories side by side, with `config` as its
/// `cupel.toml`.
fn mixed(config: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for lang in ["go", "rust"] {
        let into = dir.path().join(lang);
        fs::create_dir_all(&into).unwrap();
        copy_repo(lang, &into);
    }
    fs::write(dir.path().join("cupel.toml"), config).unwrap();
    dir
}

fn check(dir: &Path, extra: &[&str]) -> Envelope {
    let mut args = vec!["check", "--only", "static.complexity", "--format", "json"];
    args.extend(extra);
    let outcome = run_cupel(&args, dir);
    outcome.envelope().clone()
}

/// (language, kind) of every finding.
fn findings(envelope: &Envelope) -> BTreeSet<(String, String)> {
    envelope
        .findings
        .iter()
        .map(|finding| {
            let lang = finding.symbol.as_ref().unwrap().lang.to_string();
            (lang, finding.kind.clone())
        })
        .collect()
}

#[test]
fn a_go_threshold_override_changes_only_the_go_findings() {
    let defaults = check(mixed("").path(), &[]);
    let overridden = check(
        mixed("[checks.\"static.complexity\".lang.go]\nparams_max = 6\nlength_max = 100\n").path(),
        &[],
    );
    let removed: BTreeSet<_> = findings(&defaults)
        .difference(&findings(&overridden))
        .cloned()
        .collect();
    let expected: BTreeSet<(String, String)> = [("go", "params.many"), ("go", "function.long")]
        .into_iter()
        .map(|(lang, kind)| (lang.to_owned(), kind.to_owned()))
        .collect();
    assert_eq!(removed, expected);
    assert!(findings(&overridden).contains(&("rust".to_owned(), "params.many".to_owned())));
    assert!(findings(&overridden).is_subset(&findings(&defaults)));
}

#[test]
fn scope_languages_follow_the_files_in_scope() {
    let dir = mixed("");
    let both = check(dir.path(), &[]);
    let languages = |envelope: &Envelope| {
        envelope
            .run
            .scope
            .as_ref()
            .unwrap()
            .languages
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    };
    assert_eq!(languages(&both), ["go", "rust"]);
    assert_eq!(languages(&check(dir.path(), &["--paths", "go"])), ["go"]);
    assert_eq!(
        languages(&check(dir.path(), &["--paths", "rust/src/complexity.rs"])),
        ["rust"]
    );
}

#[test]
fn a_one_millisecond_timeout_on_the_fixture_tree_is_partial_with_exit_5() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("rust", dir.path());
    let outcome = run_cupel(
        &["check", "--timeout", "1ms", "--format", "json"],
        dir.path(),
    );
    assert_eq!(outcome.code, Some(5), "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope.status, Status::Partial);
    assert_eq!(envelope.exit_code, ExitCode::Partial);
    let skipped = envelope
        .checks
        .iter()
        .find(|result| result.status == CheckStatus::Skipped)
        .expect("the unstarted check is skipped");
    assert_eq!(
        skipped.skipped.as_ref().map(|skipped| skipped.reason),
        Some(SkipReason::Timeout)
    );
}
