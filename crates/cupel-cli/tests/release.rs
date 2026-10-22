// ABOUTME: Tests for scripts/release.sh, which `make dist` and the release workflow use to package the binary:
// ABOUTME: the tag-to-version check, archive names and layout, SHA256SUMS, the static check, and workflow action pins.
#![cfg(unix)]
mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use support::{jobs, workflow};
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
            "Swatinem/rust-cache",
            "actions/checkout",
            "actions/download-artifact",
            "actions/upload-artifact"
        ]
    );
    let unpinned: Vec<&&str> = actions.iter().filter(|action| !is_pinned(action)).collect();
    assert_eq!(unpinned, Vec::<&&str>::new());
}

/// The build job restores and saves the build cache per target, between installing the toolchain
/// and `make dist`, so one target's cache never overwrites another's.
#[test]
fn the_build_job_caches_each_targets_build() {
    let jobs = jobs(&release_workflow());
    let (_, lines) = jobs
        .iter()
        .find(|(name, _)| name == "build")
        .expect("a `build` job");
    let position = |wanted: &str| lines.iter().position(|line| line.contains(wanted));
    let cache = position("uses: Swatinem/rust-cache@");
    let toolchain = position("rustup toolchain install");
    let dist = position("make dist");
    assert!(
        toolchain < cache && cache < dist && cache.is_some(),
        "toolchain {toolchain:?}, cache {cache:?}, dist {dist:?}"
    );
    assert_eq!(
        lines[cache.unwrap() + 1..dist.unwrap()]
            .iter()
            .filter_map(|line| line.strip_prefix("key: "))
            .collect::<Vec<_>>(),
        ["${{ matrix.target }}"]
    );
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
    assert_eq!(setting(&workflow, "branches"), Some("[main]"));
    let (_, deploy) = workflow.split_once("\n  deploy:\n").expect("a deploy job");
    let settings: Vec<Option<&str>> = ["if", "name", "pages", "id-token"]
        .iter()
        .map(|key| setting(deploy, key))
        .collect();
    assert_eq!(
        settings,
        vec![
            Some("github.ref == 'refs/heads/main'"),
            Some("github-pages"),
            Some("write"),
            Some("write"),
        ]
    );
}

/// The value of the first `key: value` line in `yaml`, at any indentation.
fn setting<'a>(yaml: &'a str, key: &str) -> Option<&'a str> {
    yaml.lines()
        .find_map(|line| line.trim_start().strip_prefix(key)?.strip_prefix(": "))
}

/// The publish jobs: each job's name, the `secrets` output it is gated on, and its secret.
const PUBLISHERS: [(&str, &str, &str); 4] = [
    ("publish-npm", "npm", "NPM_TOKEN"),
    ("publish-pypi", "pypi", "PYPI_API_TOKEN"),
    ("publish-homebrew", "homebrew", "HOMEBREW_TAP_TOKEN"),
    ("publish-crates", "crates", "CARGO_REGISTRY_TOKEN"),
];

/// Lines `lines` lacks of `expected`.
fn missing(lines: &[String], expected: Vec<String>) -> Vec<String> {
    expected
        .into_iter()
        .filter(|line| !lines.contains(line))
        .collect()
}

/// The release job stages the downloaded archives under `target/dist`, never the repository's `dist/`,
/// which holds the installers (ADR-125): `gh release create dist/*` would try to upload `dist/homebrew`.
#[test]
fn the_release_job_stages_archives_outside_the_installers_directory() {
    let jobs = jobs(&release_workflow());
    let (_, lines) = jobs
        .iter()
        .find(|(name, _)| name == "release")
        .expect("a `release` job");
    let bare: Vec<&String> = lines
        .iter()
        .filter(|line| {
            line.split(|c: char| c.is_whitespace() || c == ':' || c == '"' || c == '\'')
                .any(|word| word == "dist" || word.starts_with("dist/"))
        })
        .collect();
    assert_eq!(bare, Vec::<&String>::new());
}

/// The `secrets` job runs on a tag push after the release and turns each publishing secret into an
/// output that says whether it is set, writing "skipped: secret X not set" when it is not.
#[test]
fn the_secrets_job_probes_each_publishing_secret_on_a_tag_push() {
    let jobs = jobs(&release_workflow());
    let (_, lines) = jobs
        .iter()
        .find(|(name, _)| name == "secrets")
        .expect("a `secrets` job");
    let mut expected = vec![
        "needs: release".to_owned(),
        "if: github.event_name == 'push' && github.ref_type == 'tag'".to_owned(),
        "echo \"::notice title=publish-$1::skipped: secret $2 not set\"".to_owned(),
    ];
    expected.extend(PUBLISHERS.iter().flat_map(|(_, output, secret)| {
        [
            format!("{output}: ${{{{ steps.probe.outputs.{output} }}}}"),
            format!("{secret}: ${{{{ secrets.{secret} }}}}"),
            format!("probe {output} {secret}"),
        ]
    }));
    assert_eq!(missing(lines, expected), Vec::<String>::new());
}

/// Every publish job runs only when the `secrets` job found its secret, and reads that secret.
#[test]
fn every_publish_job_is_gated_on_its_secret() {
    let jobs = jobs(&release_workflow());
    let ungated: Vec<(&str, Vec<String>)> = PUBLISHERS
        .iter()
        .map(|(job, output, secret)| {
            let lines = jobs
                .iter()
                .find(|(name, _)| name == job)
                .map(|(_, lines)| lines.clone())
                .unwrap_or_default();
            let expected = vec![
                "needs: secrets".to_owned(),
                format!("if: needs.secrets.outputs.{output} == 'true'"),
            ];
            let mut absent = missing(&lines, expected);
            if !lines
                .iter()
                .any(|line| line.ends_with(&format!("${{{{ secrets.{secret} }}}}")))
            {
                absent.push(format!("secrets.{secret}"));
            }
            (*job, absent)
        })
        .filter(|(_, absent)| !absent.is_empty())
        .collect();
    assert_eq!(ungated, Vec::<(&str, Vec<String>)>::new());
}

/// Nothing but the `secrets` job and the publish job a secret gates reads that secret.
#[test]
fn only_its_publish_job_and_the_probe_read_each_secret() {
    let jobs = jobs(&release_workflow());
    let readers: Vec<(String, String)> = jobs
        .iter()
        .flat_map(|(name, lines)| {
            lines
                .iter()
                .filter_map(|line| line.split_once("${{ secrets.").map(|(_, rest)| rest))
                .map(|rest| (name.clone(), rest.trim_end_matches(" }}").to_owned()))
                .collect::<Vec<_>>()
        })
        .filter(|(name, _)| name != "secrets")
        .collect();
    let expected: Vec<(String, String)> = PUBLISHERS
        .iter()
        .map(|(job, _, secret)| ((*job).to_owned(), (*secret).to_owned()))
        .collect();
    assert_eq!(readers, expected);
}

/// The Windows build is a release target like the others (ADR-137): no build may fail without
/// failing the release.
#[test]
fn every_release_build_holds_back_the_release_when_it_fails() {
    let jobs = jobs(&release_workflow());
    let (_, lines) = jobs
        .iter()
        .find(|(name, _)| name == "build")
        .expect("a `build` job");
    let lenient: Vec<&String> = lines
        .iter()
        .filter(|line| line.contains("continue-on-error") || line.contains("best-effort"))
        .collect();
    assert_eq!(lenient, Vec::<&String>::new());
}

/// CI's `windows` job (ADR-137) compiles the workspace for the Windows target and runs the tests
/// that need no Unix tools, both through make.
#[test]
fn ci_compiles_and_tests_the_windows_target() {
    let jobs = jobs(&workflow("ci.yml"));
    let lines = jobs
        .iter()
        .find(|(name, _)| name == "windows")
        .map(|(_, lines)| lines.clone())
        .unwrap_or_default();
    let expected = vec![
        "runs-on: windows-latest".to_owned(),
        "run: choco install make --version 4.4.1 --no-progress -y".to_owned(),
        "run: make windows-check".to_owned(),
        "run: make windows-test".to_owned(),
    ];
    assert_eq!(missing(&lines, expected), Vec::<String>::new());
}

/// The files a release's version is written in, relative to the repository root.
const VERSIONED: [&str; 4] = [
    "Cargo.toml",
    "Cargo.lock",
    "dist/npm/package.json",
    "dist/pipx/cupel.py",
];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Each version the files under `root` name the release by, as `(file, version)`: the workspace
/// package's, each workspace crate's in the lock file, the npm package's, and the pipx module's.
fn named_versions(root: &Path) -> Vec<(String, String)> {
    let read = |file: &str| fs::read_to_string(root.join(file)).unwrap();
    let quoted = |line: &str, prefix: &str| {
        line.strip_prefix(prefix)
            .and_then(|rest| rest.split('"').nth(1))
            .map(str::to_owned)
    };
    let mut versions = Vec::new();
    let manifest = read("Cargo.toml");
    let package = manifest
        .split_once("[workspace.package]\n")
        .expect("a [workspace.package] table")
        .1;
    versions.extend(
        package
            .lines()
            .find_map(|line| quoted(line, "version = "))
            .map(|version| ("Cargo.toml".to_owned(), version)),
    );
    let lock = read("Cargo.lock");
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if let Some(name) = quoted(line, "name = ").filter(|name| name.starts_with("cupel-")) {
            let version = lines.next().and_then(|next| quoted(next, "version = "));
            versions.push((format!("Cargo.lock {name}"), version.unwrap_or_default()));
        }
    }
    let npm: serde_json::Value = serde_json::from_str(&read("dist/npm/package.json")).unwrap();
    versions.push((
        "dist/npm/package.json".to_owned(),
        npm["version"].as_str().unwrap_or_default().to_owned(),
    ));
    versions.extend(
        read("dist/pipx/cupel.py")
            .lines()
            .find_map(|line| quoted(line, "__version__ = "))
            .map(|version| ("dist/pipx/cupel.py".to_owned(), version)),
    );
    versions
}

/// The workspace, its lock file, and both wrappers name the same release, the crate's.
#[test]
fn every_file_that_names_the_release_carries_the_crate_version() {
    let versions = named_versions(&repository());
    assert_eq!(versions.len(), 13, "{versions:?}");
    let different: Vec<&(String, String)> = versions
        .iter()
        .filter(|(_, version)| version != VERSION)
        .collect();
    assert_eq!(different, Vec::<&(String, String)>::new());
}

/// A copy of the release script and the versioned files in a directory of their own.
fn versioned_copy() -> TempDir {
    let dir = TempDir::new().unwrap();
    for file in VERSIONED.iter().chain(&["scripts/release.sh"]) {
        let to = dir.path().join(file);
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::copy(repository().join(file), to).unwrap();
    }
    dir
}

fn release_in(dir: &Path, args: &[&str]) -> Output {
    Command::new("sh")
        .arg(dir.join("scripts/release.sh"))
        .args(args)
        .output()
        .expect("sh runs scripts/release.sh")
}

/// `version --set` writes the new version into every versioned file and changes nothing else.
#[test]
fn version_set_rewrites_the_version_in_every_file_that_names_it() {
    let dir = versioned_copy();
    let output = release_in(dir.path(), &["version", "--set", "9.8.7"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "9.8.7\n");
    let versions = named_versions(dir.path());
    assert_eq!(versions.len(), 13, "{versions:?}");
    assert!(
        versions.iter().all(|(_, version)| version == "9.8.7"),
        "{versions:?}"
    );
    let changed: Vec<String> = VERSIONED
        .iter()
        .flat_map(|file| {
            let before = fs::read_to_string(repository().join(file)).unwrap();
            let after = fs::read_to_string(dir.path().join(file)).unwrap();
            assert_eq!(before.lines().count(), after.lines().count(), "{file}");
            before
                .lines()
                .zip(after.lines())
                .filter(|(was, is)| was != is)
                .map(|(was, is)| format!("{file}: {}", was.replace(VERSION, "9.8.7") == is))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(changed.len(), 13, "{changed:?}");
    assert!(
        changed.iter().all(|line| line.ends_with(": true")),
        "{changed:?}"
    );
    let shown = release_in(dir.path(), &["version"]);
    assert_eq!(stdout(&shown), "9.8.7\n");
}

/// `version --set` takes a plain `MAJOR.MINOR.PATCH` and writes nothing for anything else.
#[test]
fn version_set_refuses_what_is_not_a_version() {
    let dir = versioned_copy();
    let refused: Vec<(Option<i32>, String)> = ["v1.2.3", "1.2", "1.2.3-rc.1", ""]
        .iter()
        .map(|text| {
            let output = release_in(dir.path(), &["version", "--set", text]);
            (output.status.code(), stderr(&output))
        })
        .collect();
    let expected: Vec<(Option<i32>, String)> = ["v1.2.3", "1.2", "1.2.3-rc.1", ""]
        .iter()
        .map(|text| {
            (
                Some(1),
                format!(
                    "release.sh: {text:?} is not a version: pass MAJOR.MINOR.PATCH, such as 0.2.0\n"
                ),
            )
        })
        .collect();
    assert_eq!(refused, expected);
    assert!(named_versions(dir.path()).iter().all(|(_, v)| v == VERSION));
}

/// `make version V=X.Y.Z` is the script's `version --set`, and the README's Makefile table lists it.
#[test]
fn make_version_sets_the_version_through_the_release_script() {
    let output = Command::new("make")
        .args(["--no-print-directory", "-n", "version", "V=9.8.7"])
        .current_dir(repository())
        .output()
        .expect("make runs");
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "scripts/release.sh version --set 9.8.7\n");
    let readme = fs::read_to_string(repository().join("README.md")).unwrap();
    assert!(
        readme
            .lines()
            .any(|line| line.starts_with("| `make version V=X.Y.Z` |")),
        "the README's Makefile table lists `make version`"
    );
}
