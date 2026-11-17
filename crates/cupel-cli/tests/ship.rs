// ABOUTME: Tests for scripts/ship.sh and its make targets (ADR-238), which release and publish from the owner's machine:
// ABOUTME: the release's preconditions, the build per target, the tokens from the environment, and that no workflow publishes.
#![cfg(unix)]
mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Exit code and stderr, the shape every refusal is checked in.
fn refusal(output: &Output) -> (Option<i32>, String) {
    (output.status.code(), stderr(output))
}

/// A git repository of version 1.2.3 holding copies of the release scripts and dist/targets.toml, with
/// one commit and no tag.
fn release_repository() -> TempDir {
    let dir = TempDir::new().unwrap();
    for file in ["scripts/ship.sh", "scripts/release.sh", "dist/targets.toml"] {
        let to = dir.path().join(file);
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::copy(repository().join(file), to).unwrap();
    }
    fs::write(
        dir.path().join("Cargo.toml"),
        "[workspace.package]\nversion = \"1.2.3\"\n",
    )
    .unwrap();
    for args in [
        &["init", "--quiet", "--initial-branch", "main"][..],
        &["add", "."],
        &["commit", "--quiet", "-m", "c"],
    ] {
        support::git(dir.path(), args);
    }
    dir
}

/// scripts/ship.sh in `dir`, without the Python Package Index's token in its environment.
fn ship(dir: &Path, args: &[&str]) -> Output {
    Command::new("sh")
        .arg(dir.join("scripts/ship.sh"))
        .args(args)
        .current_dir(dir)
        .env_remove("PYPI_TOKEN")
        .output()
        .expect("sh runs scripts/ship.sh")
}

fn head(dir: &Path) -> String {
    support::git(dir, &["rev-parse", "HEAD"]).trim().to_owned()
}

#[test]
fn release_refuses_what_is_not_a_version() {
    let dir = release_repository();
    assert_eq!(
        refusal(&ship(dir.path(), &["release", "1.2"])),
        (
            Some(1),
            "ship.sh: \"1.2\" is not a version: pass MAJOR.MINOR.PATCH, such as 0.3.0\n".to_owned()
        )
    );
}

/// A release is built from exactly the tagged commit: a change or an untracked file would ship in
/// none of the tag's sources.
#[test]
fn release_refuses_a_tree_with_changes() {
    let dir = release_repository();
    support::git(dir.path(), &["tag", "-a", "v1.2.3", "-m", "Cupel 1.2.3"]);
    fs::write(dir.path().join("extra"), "").unwrap();
    assert_eq!(
        refusal(&ship(dir.path(), &["release", "1.2.3"])),
        (
            Some(1),
            "ship.sh: the tree has changes; release from a clean checkout of v1.2.3:\n?? extra\n"
                .to_owned()
        )
    );
}

#[test]
fn release_refuses_a_version_with_no_tag() {
    let dir = release_repository();
    assert_eq!(
        refusal(&ship(dir.path(), &["release", "1.2.3"])),
        (
            Some(1),
            "ship.sh: there is no tag v1.2.3; tag the release commit first: git tag -a v1.2.3 -m \"Cupel 1.2.3\"\n"
                .to_owned()
        )
    );
}

#[test]
fn release_refuses_a_tag_that_is_not_the_checked_out_commit() {
    let dir = release_repository();
    support::git(dir.path(), &["tag", "-a", "v1.2.3", "-m", "Cupel 1.2.3"]);
    let tagged = head(dir.path());
    support::git(
        dir.path(),
        &["commit", "--quiet", "--allow-empty", "-m", "later"],
    );
    let checked_out = head(dir.path());
    assert_eq!(
        refusal(&ship(dir.path(), &["release", "1.2.3"])),
        (
            Some(1),
            format!(
                "ship.sh: v1.2.3 is {tagged}, not the checked-out commit {checked_out}; check out the tag\n"
            )
        )
    );
}

#[test]
fn release_refuses_a_version_other_than_the_crate_version() {
    let dir = release_repository();
    support::git(dir.path(), &["tag", "-a", "v1.2.4", "-m", "Cupel 1.2.4"]);
    assert_eq!(
        refusal(&ship(dir.path(), &["release", "1.2.4"])),
        (
            Some(1),
            "release.sh: tag v1.2.4 does not match the crate version 1.2.3 (expected v1.2.3)\n"
                .to_owned()
        )
    );
}

/// The Python Package Index upload reads its token only from the environment ship.sh is started in, which
/// `op run --env-file=release.env.tpl` fills from 1Password; without it nothing is attempted. npm needs
/// none: the owner signs in through the browser for each publish (ADR-238).
#[test]
fn publishing_to_pypi_needs_its_token_in_the_environment() {
    let dir = release_repository();
    assert_eq!(
        refusal(&ship(dir.path(), &["publish", "pypi", "1.2.3"])),
        (
            Some(1),
            "ship.sh: PYPI_TOKEN is not set; run it with the token from 1Password: op run --env-file=release.env.tpl -- make publish-pypi V=1.2.3\n"
                .to_owned()
        )
    );
}

/// crates.io is not a route while another project holds a crate name (ADR-146).
#[test]
fn publishing_names_only_npm_pypi_and_homebrew() {
    let dir = release_repository();
    assert_eq!(
        refusal(&ship(dir.path(), &["publish", "crates", "1.2.3"])),
        (
            Some(2),
            "ship.sh: no registry \"crates\": publish to npm, pypi, or homebrew (crates.io stays unpublished, ADR-146)\n"
                .to_owned()
        )
    );
}

/// What `make -n <args>` would run in the repository.
fn make_n(args: &[&str]) -> String {
    let output = Command::new("make")
        .args(["--no-print-directory", "-n"])
        .args(args)
        .current_dir(repository())
        .output()
        .expect("make runs");
    assert!(output.status.success(), "{}", stderr(&output));
    stdout(&output)
}

/// Each step is a make target of its own, `publish` is the three registries in turn, and `DRY=1`
/// passes `--dry-run`.
#[test]
fn the_make_targets_run_each_step_through_ship_sh() {
    assert_eq!(
        (
            make_n(&["release", "V=9.8.7"]),
            make_n(&["release", "V=9.8.7", "DRY=1"]),
            make_n(&["publish", "V=9.8.7", "DRY=1"]),
            make_n(&["publish-pypi", "V=9.8.7"]),
            make_n(&["verify-release", "V=9.8.7"]),
        ),
        (
            "scripts/ship.sh release 9.8.7\n".to_owned(),
            "scripts/ship.sh --dry-run release 9.8.7\n".to_owned(),
            "scripts/ship.sh --dry-run publish npm 9.8.7\n\
             scripts/ship.sh --dry-run publish pypi 9.8.7\n\
             scripts/ship.sh --dry-run publish homebrew 9.8.7\n"
                .to_owned(),
            "scripts/ship.sh publish pypi 9.8.7\n".to_owned(),
            "scripts/ship.sh verify 9.8.7\n".to_owned(),
        )
    );
}

/// dist/targets.toml's triples, in file order.
fn triples() -> Vec<String> {
    fs::read_to_string(repository().join("dist/targets.toml"))
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("triple = \""))
        .map(|rest| rest.trim_end_matches('"').to_owned())
        .collect()
}

/// The Linux and Windows archives cross-compile with cargo-zigbuild; both macOS archives build with
/// Apple's toolchain, so a release builds on a Mac.
#[test]
fn dist_builds_linux_and_windows_with_zig_and_macos_natively() {
    let builds: Vec<(String, bool)> = triples()
        .into_iter()
        .map(|triple| {
            let commands = make_n(&["dist", &format!("TARGET={triple}")]);
            let zig = commands
                .lines()
                .next()
                .is_some_and(|line| line.contains(" zigbuild "));
            (triple, zig)
        })
        .collect();
    assert_eq!(
        builds,
        [
            ("x86_64-unknown-linux-musl", true),
            ("aarch64-unknown-linux-musl", true),
            ("x86_64-apple-darwin", false),
            ("aarch64-apple-darwin", false),
            ("x86_64-pc-windows-gnu", true),
        ]
        .map(|(triple, zig)| (triple.to_owned(), zig))
    );
}

/// What would publish from a workflow: a release, a registry upload, or a push to the tap.
const PUBLISHING: [&str; 7] = [
    "gh release",
    "npm publish",
    "twine",
    "gh-action-pypi-publish",
    "cargo publish",
    "homebrew/publish.sh",
    "homebrew-cupel",
];

/// Each `<workflow file>: <marker>` for a PUBLISHING marker a file in `dir` names.
fn publishing_in(dir: &Path) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        found.extend(
            PUBLISHING
                .iter()
                .filter(|marker| text.contains(*marker))
                .map(|marker| format!("{name}: {marker}")),
        );
    }
    found
}

/// Nothing publishes from GitHub Actions (ADR-238): no workflow creates a release or uploads to a
/// registry, and the release workflow is gone.
#[test]
fn no_workflow_publishes() {
    let workflows = repository().join(".github/workflows");
    assert_eq!(
        (
            workflows.join("release.yml").exists(),
            publishing_in(&workflows)
        ),
        (false, Vec::<String>::new())
    );
}

/// release.env.tpl holds 1Password references and no values: one per token ship.sh reads, the Python Package Index's alone.
#[test]
fn the_release_environment_template_holds_only_1password_references() {
    let template = fs::read_to_string(repository().join("release.env.tpl")).unwrap();
    let entries: Vec<(String, bool)> = template
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (name, value) = line.split_once('=').unwrap_or((line, ""));
            (name.to_owned(), value.starts_with("op://"))
        })
        .collect();
    let script = fs::read_to_string(repository().join("scripts/ship.sh")).unwrap();
    let read: Vec<bool> = entries
        .iter()
        .map(|(name, _)| script.contains(&format!("${{{name}")))
        .collect();
    assert_eq!(
        (entries, read),
        (vec![("PYPI_TOKEN".to_owned(), true)], vec![true])
    );
}
