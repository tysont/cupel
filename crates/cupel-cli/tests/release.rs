// ABOUTME: Tests for scripts/release.sh, which `make dist` and the release workflow use to package the binary:
// ABOUTME: the tag-to-version check, archive names and layout, SHA256SUMS, the static check, and workflow action pins.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/release.sh")
}

fn release(args: &[&str]) -> Output {
    Command::new("sh")
        .arg(script())
        .args(args)
        .output()
        .expect("sh runs scripts/release.sh")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A stand-in binary: packaging copies whatever file it is given.
fn binary(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, "#!/bin/sh\necho stand-in\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// Packages a stand-in binary for `target` into `out` and returns the archive's path.
fn package(dir: &Path, target: &str, out: &Path) -> PathBuf {
    let name = if target.contains("windows") {
        "cupel.exe"
    } else {
        "cupel"
    };
    let bin = binary(dir, name);
    let output = release(&[
        "package",
        bin.to_str().unwrap(),
        target,
        out.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "package: {}", stderr(&output));
    PathBuf::from(stdout(&output).trim())
}

/// An archive packaged for `target` into `out`, a directory in `dir`, which lives as long as this does.
struct Packaged {
    dir: TempDir,
    out: PathBuf,
    archive: PathBuf,
}

fn packaged(target: &str) -> Packaged {
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("dist");
    let archive = package(dir.path(), target, &out);
    Packaged { dir, out, archive }
}

fn tar_entries(archive: &Path) -> Vec<String> {
    let output = Command::new("tar")
        .arg("-tzf")
        .arg(archive)
        .output()
        .unwrap();
    assert!(output.status.success(), "tar -tzf: {}", stderr(&output));
    let mut entries: Vec<String> = stdout(&output)
        .lines()
        .filter(|line| !line.ends_with('/'))
        .map(str::to_owned)
        .collect();
    entries.sort();
    entries
}

#[test]
fn version_prints_the_crate_version() {
    let output = release(&["version"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), format!("{VERSION}\n"));
}

#[test]
fn version_accepts_the_tag_of_the_crate_version() {
    let output = release(&["version", &format!("v{VERSION}")]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), format!("{VERSION}\n"));
}

#[test]
fn version_rejects_a_tag_that_differs_from_the_crate_version() {
    let output = release(&["version", "v999.0.0"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        format!(
            "release.sh: tag v999.0.0 does not match the crate version {VERSION} (expected v{VERSION})\n"
        )
    );
}

#[test]
fn package_writes_a_tarball_of_the_binary_license_and_readme() {
    let Packaged {
        dir: _dir,
        out,
        archive,
    } = packaged("x86_64-unknown-linux-musl");
    let top = format!("cupel-{VERSION}-x86_64-unknown-linux-musl");
    assert_eq!(archive, out.join(format!("{top}.tar.gz")));
    assert_eq!(
        tar_entries(&archive),
        vec![
            format!("{top}/LICENSE"),
            format!("{top}/README.md"),
            format!("{top}/cupel"),
        ]
    );
    let output = release(&["verify", archive.to_str().unwrap()]);
    assert!(output.status.success(), "verify: {}", stderr(&output));
    let leftovers: Vec<_> = fs::read_dir(&out)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(
        leftovers,
        vec![std::ffi::OsString::from(format!("{top}.tar.gz"))]
    );
}

#[test]
fn package_writes_a_zip_for_a_windows_target() {
    let Packaged {
        dir: _dir,
        out,
        archive,
    } = packaged("x86_64-pc-windows-msvc");
    assert_eq!(
        archive,
        out.join(format!("cupel-{VERSION}-x86_64-pc-windows-msvc.zip"))
    );
    let output = release(&["verify", archive.to_str().unwrap()]);
    assert!(output.status.success(), "verify: {}", stderr(&output));
}

#[test]
fn verify_rejects_an_archive_without_the_license() {
    let dir = TempDir::new().unwrap();
    let top = format!("cupel-{VERSION}-aarch64-apple-darwin");
    let staged = dir.path().join(&top);
    fs::create_dir(&staged).unwrap();
    binary(&staged, "cupel");
    fs::write(staged.join("README.md"), "readme\n").unwrap();
    let archive = dir.path().join(format!("{top}.tar.gz"));
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(dir.path())
        .arg(&top)
        .status()
        .unwrap();
    assert!(status.success());
    let output = release(&["verify", archive.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).starts_with(&format!(
            "release.sh: {top}.tar.gz holds\n{top}/README.md\n{top}/cupel\n"
        )),
        "{}",
        stderr(&output)
    );
}

#[test]
fn sums_lists_the_sha256_of_every_archive_by_name() {
    let Packaged { dir, out, .. } = packaged("x86_64-unknown-linux-musl");
    package(dir.path(), "aarch64-unknown-linux-musl", &out);
    let output = release(&["sums", out.to_str().unwrap()]);
    assert!(output.status.success(), "sums: {}", stderr(&output));
    let sums = fs::read_to_string(out.join("SHA256SUMS")).unwrap();
    let names: Vec<&str> = sums
        .lines()
        .map(|line| {
            let (hash, name) = line.split_once("  ").expect("`<hash>  <name>`");
            assert_eq!(hash.len(), 64, "{line}");
            assert!(hash.bytes().all(|b| b.is_ascii_hexdigit()), "{line}");
            name
        })
        .collect();
    assert_eq!(
        names,
        vec![
            format!("cupel-{VERSION}-aarch64-unknown-linux-musl.tar.gz"),
            format!("cupel-{VERSION}-x86_64-unknown-linux-musl.tar.gz"),
        ]
    );
}

#[test]
fn static_rejects_a_dynamically_linked_binary() {
    let output = release(&["static", env!("CARGO_BIN_EXE_cupel")]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("is not a statically linked executable"),
        "{}",
        stderr(&output)
    );
}

fn workflow(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.github/workflows")
            .join(name),
    )
    .unwrap()
}

fn release_workflow() -> String {
    workflow("release.yml")
}

/// dist/targets.toml (ADR-125): each `[[target]]`'s triple and archive format, in file order.
fn targets() -> Vec<(String, String)> {
    let toml =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/targets.toml"))
            .unwrap();
    let document: toml_edit::DocumentMut = toml.parse().unwrap();
    document["target"]
        .as_array_of_tables()
        .expect("[[target]] tables")
        .iter()
        .map(|table| {
            let text = |key: &str| table[key].as_str().unwrap().to_owned();
            (text("triple"), text("format"))
        })
        .collect()
}

/// The workflow builds exactly the targets dist/targets.toml lists, which the installers read.
#[test]
fn release_workflow_builds_the_targets_the_installers_expect() {
    let mut built: Vec<String> = release_workflow()
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- target: "))
        .map(str::to_owned)
        .collect();
    built.sort();
    let mut listed: Vec<String> = targets().into_iter().map(|(triple, _)| triple).collect();
    listed.sort();
    assert_eq!(built, listed);
}

/// Each target's archive is named as dist/targets.toml's `archive` template says, so an installer
/// that renders the template downloads what the workflow published.
#[test]
fn package_names_each_archive_as_the_installers_expect() {
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("dist");
    let named: Vec<String> = targets()
        .iter()
        .map(|(triple, _)| {
            package(dir.path(), triple, &out)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let expected: Vec<String> = targets()
        .iter()
        .map(|(triple, format)| format!("cupel-{VERSION}-{triple}.{format}"))
        .collect();
    assert_eq!(named, expected);
}

/// Whether `action` (what follows `uses: `) names a 40-character commit with its release beside it,
/// as in `actions/checkout@<sha> # v7.0.1`.
fn is_pinned(action: &str) -> bool {
    let Some((_, pin)) = action.split_once('@') else {
        return false;
    };
    let Some((sha, release)) = pin.split_once(" # v") else {
        return false;
    };
    sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()) && release.split('.').count() == 3
}

/// What follows `uses: ` on each line of `workflow` that names an action.
fn actions(workflow: &str) -> Vec<&str> {
    workflow
        .lines()
        .filter_map(|line| line.split_once("uses: ").map(|(_, action)| action))
        .collect()
}

/// The distinct names (before `@`) of `actions`, sorted.
fn action_names<'a>(actions: &[&'a str]) -> Vec<&'a str> {
    let mut names: Vec<&str> = actions
        .iter()
        .filter_map(|action| action.split_once('@').map(|(name, _)| name))
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// Every action is pinned to a commit, with the release it is in a comment beside it.
#[test]
fn release_workflow_pins_every_action_to_a_commit() {
    let workflow = release_workflow();
    let actions = actions(&workflow);
    let names = action_names(&actions);
    assert_eq!(
        names,
        vec![
            "actions/checkout",
            "actions/download-artifact",
            "actions/upload-artifact"
        ]
    );
    let unpinned: Vec<&&str> = actions.iter().filter(|action| !is_pinned(action)).collect();
    assert_eq!(unpinned, Vec::<&&str>::new());
}

/// The pages workflow (ADR-127) holds its actions to the release workflow's rule.
#[test]
fn pages_workflow_pins_every_action_to_a_commit() {
    let workflow = workflow("pages.yml");
    let actions = actions(&workflow);
    assert_eq!(
        action_names(&actions),
        vec![
            "actions/checkout",
            "actions/deploy-pages",
            "actions/upload-pages-artifact"
        ]
    );
    let unpinned: Vec<&&str> = actions.iter().filter(|action| !is_pinned(action)).collect();
    assert_eq!(unpinned, Vec::<&&str>::new());
}

/// The pages workflow deploys from `main` only: a push elsewhere does not start it, and a
/// `workflow_dispatch` on another branch builds the site but skips the deploy job.
#[test]
fn pages_workflow_deploys_main_only() {
    let workflow = workflow("pages.yml");
    assert!(
        workflow.contains("\n  push:\n    branches: [main]\n"),
        "{workflow}"
    );
    let (_, deploy) = workflow.split_once("\n  deploy:\n").expect("a deploy job");
    assert!(
        deploy.contains("\n    if: github.ref == 'refs/heads/main'\n"),
        "{deploy}"
    );
    assert!(deploy.contains("\n      name: github-pages\n"), "{deploy}");
    assert!(deploy.contains("\n      pages: write\n"), "{deploy}");
    assert!(deploy.contains("\n      id-token: write\n"), "{deploy}");
}
