// ABOUTME: Tests for scripts/release.sh, which `make dist` and the release workflow use to package the binary:
// ABOUTME: the tag-to-version check, the archive name and layout per target, SHA256SUMS, and the static-binary check.
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
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("dist");
    let archive = package(dir.path(), "x86_64-unknown-linux-musl", &out);
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
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("dist");
    let archive = package(dir.path(), "x86_64-pc-windows-msvc", &out);
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
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("dist");
    package(dir.path(), "x86_64-unknown-linux-musl", &out);
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

fn release_workflow() -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/release.yml"),
    )
    .unwrap()
}

/// Design §10's platforms: Linux x86_64 and aarch64 (musl), macOS arm64 and x86_64, Windows x86_64.
#[test]
fn release_workflow_builds_the_design_targets() {
    let mut targets: Vec<String> = release_workflow()
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- target: "))
        .map(str::to_owned)
        .collect();
    targets.sort();
    assert_eq!(
        targets,
        vec![
            "aarch64-apple-darwin",
            "aarch64-unknown-linux-musl",
            "x86_64-apple-darwin",
            "x86_64-pc-windows-msvc",
            "x86_64-unknown-linux-musl",
        ]
    );
}

/// Every action is pinned to a commit, with the release it is in a comment beside it.
#[test]
fn release_workflow_pins_every_action_to_a_commit() {
    let unpinned: Vec<String> = release_workflow()
        .lines()
        .filter_map(|line| {
            line.split_once("uses: ")
                .map(|(_, action)| action.to_owned())
        })
        .filter(|action| {
            let Some((_, pin)) = action.split_once('@') else {
                return true;
            };
            let Some((sha, release)) = pin.split_once(" # v") else {
                return true;
            };
            sha.len() != 40
                || !sha.bytes().all(|b| b.is_ascii_hexdigit())
                || release.split('.').count() != 3
        })
        .collect();
    assert_eq!(unpinned, Vec::<String>::new());
}
