// ABOUTME: Tests for `cupel_core::git` against real temporary repositories: default branch, HEAD,
// ABOUTME: merge bases, and changed files between revisions and the working tree (ADR-002).
mod support;

use cupel_core::git::Git;
use support::GitRepo;

#[test]
fn a_directory_outside_any_repository_has_no_git() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(Git::open(dir.path()), None);
}

#[test]
fn a_local_main_branch_wins_over_the_checked_out_branch() {
    let Some(repo) = GitRepo::new() else { return };
    let git = Git::open(&repo.root()).unwrap();
    assert_eq!(git.default_branch().as_deref(), Some("main"), "unborn HEAD");
    repo.commit("first");
    repo.git(&["checkout", "--quiet", "-b", "feature"]);
    assert_eq!(git.default_branch().as_deref(), Some("main"));
}

#[test]
fn head_reports_sha_branch_and_dirtiness() {
    let Some(repo) = GitRepo::new() else { return };
    let git = Git::open(&repo.root()).unwrap();
    let unborn = git.head();
    assert_eq!(unborn.sha, None);
    assert_eq!(unborn.branch.as_deref(), Some("main"));

    repo.write("a.rs", "fn a() {}\n");
    let sha = repo.commit("first");
    let clean = git.head();
    assert_eq!(clean.sha.as_deref(), Some(sha.as_str()));
    assert!(!clean.dirty);

    repo.write("b.rs", "fn b() {}\n");
    assert!(git.head().dirty, "an untracked file makes the tree dirty");

    repo.git(&["checkout", "--quiet", "--detach"]);
    assert_eq!(git.head().branch, None);
}

#[test]
fn changed_since_includes_committed_staged_modified_and_untracked_files() {
    let Some(repo) = GitRepo::new() else { return };
    repo.write("same.rs", "fn same() {}\n");
    repo.write("edited.rs", "fn edited() {}\n");
    repo.write(".gitignore", "ignored.rs\n");
    let base = repo.commit("base");
    repo.git(&["checkout", "--quiet", "-b", "feature"]);
    repo.write("committed.rs", "fn committed() {}\n");
    repo.commit("on the branch");
    repo.write("staged.rs", "fn staged() {}\n");
    repo.git(&["add", "staged.rs"]);
    repo.write("edited.rs", "fn edited() { 1; }\n");
    repo.write("dir/untracked.rs", "fn untracked() {}\n");
    repo.write("ignored.rs", "fn ignored() {}\n");

    let git = Git::open(&repo.root()).unwrap();
    let merge_base = git.merge_base("main", "HEAD").unwrap();
    assert_eq!(merge_base, base);
    assert_eq!(
        git.changed_since(&merge_base).unwrap(),
        ["committed.rs", "dir/untracked.rs", "edited.rs", "staged.rs"]
    );
}

#[test]
fn changed_between_lists_only_files_that_differ_between_two_commits() {
    let Some(repo) = GitRepo::new() else { return };
    repo.write("a.rs", "fn a() {}\n");
    let first = repo.commit("first");
    repo.write("b.rs", "fn b() {}\n");
    let second = repo.commit("second");
    repo.write("c.rs", "fn c() {}\n");

    let git = Git::open(&repo.root()).unwrap();
    assert_eq!(git.changed_between(&first, &second).unwrap(), ["b.rs"]);
    assert_eq!(git.changed_since(&second).unwrap(), ["c.rs"]);
}

#[test]
fn paths_are_relative_to_a_root_below_the_top_of_the_repository() {
    let Some(repo) = GitRepo::new() else { return };
    repo.write("outside.rs", "fn o() {}\n");
    repo.write("sub/inside.rs", "fn i() {}\n");
    let base = repo.commit("base");
    repo.write("outside.rs", "fn o() { 1; }\n");
    repo.write("sub/inside.rs", "fn i() { 1; }\n");
    repo.write("sub/new.rs", "fn n() {}\n");

    let git = Git::open(&repo.root().join("sub")).unwrap();
    assert_eq!(git.changed_since(&base).unwrap(), ["inside.rs", "new.rs"]);
}

#[test]
fn an_unknown_revision_is_an_error_naming_it() {
    let Some(repo) = GitRepo::new() else { return };
    repo.commit("first");
    let git = Git::open(&repo.root()).unwrap();
    let error = git.commit("no-such-ref").unwrap_err();
    assert!(error.message.contains("`no-such-ref`"), "{error}");
    assert!(git.commit("HEAD").is_ok());
}
