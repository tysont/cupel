// ABOUTME: Integration tests for the installers under dist/ (ADR-125): cargo-binstall metadata, install.sh, the
// ABOUTME: Homebrew formula, and the npm and pipx wrappers, each run against a fake release on a local HTTP server.
#![cfg(unix)]
mod support;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use serde_json::{Value, json};
use support::{Outcome, Repo, installed, workspace};
use tempfile::TempDir;
use toml_edit::{DocumentMut, Item, Table};

/// The version the fake release publishes: the binary's own, which its `--version` prints.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The release `tests/fixtures/SHA256SUMS` lists.
const FIXTURE_VERSION: &str = "0.1.0";

fn read_toml(relative: &str) -> DocumentMut {
    fs::read_to_string(workspace().join(relative))
        .unwrap()
        .parse()
        .unwrap_or_else(|error| panic!("{relative} is TOML: {error}"))
}

fn text(table: &Table, key: &str) -> String {
    table
        .get(key)
        .and_then(Item::as_str)
        .unwrap_or_else(|| panic!("`{key}` is a string"))
        .to_string()
}

/// One `[[target]]` of dist/targets.toml.
struct Target {
    triple: String,
    os: String,
    arch: String,
    format: String,
}

impl Target {
    fn exe(&self) -> &'static str {
        if self.os == "windows" { ".exe" } else { "" }
    }

    /// The target's key in the wrappers' tables: `<os>-<arch>` in Rust's spelling.
    fn key(&self) -> String {
        format!("{}-{}", self.os, self.arch)
    }

    /// What `uname -s` and `uname -m` print on the target.
    fn uname(&self) -> (&'static str, &str) {
        match (self.os.as_str(), self.arch.as_str()) {
            ("macos", "aarch64") => ("Darwin", "arm64"),
            ("macos", arch) => ("Darwin", arch),
            (_, arch) => ("Linux", arch),
        }
    }
}

/// dist/targets.toml: the names of a release's files, and every target.
struct Targets {
    archive: String,
    dir: String,
    checksums: String,
    all: Vec<Target>,
}

impl Targets {
    fn read() -> Self {
        let document = read_toml("dist/targets.toml");
        let root = document.as_table();
        let targets = document["target"]
            .as_array_of_tables()
            .expect("[[target]] tables")
            .iter()
            .map(|table| Target {
                triple: text(table, "triple"),
                os: text(table, "os"),
                arch: text(table, "arch"),
                format: text(table, "format"),
            })
            .collect();
        Self {
            archive: text(root, "archive"),
            dir: text(root, "dir"),
            checksums: text(root, "checksums"),
            all: targets,
        }
    }

    fn fill(template: &str, version: &str, target: &Target) -> String {
        template
            .replace("{version}", version)
            .replace("{target}", &target.triple)
            .replace("{format}", &target.format)
    }

    fn archive(&self, version: &str, target: &Target) -> String {
        Self::fill(&self.archive, version, target)
    }

    fn dir(&self, target: &Target) -> String {
        Self::fill(&self.dir, VERSION, target)
    }

    /// The target this test runs on.
    fn host(&self) -> &Target {
        self.all
            .iter()
            .find(|t| t.os == std::env::consts::OS && t.arch == std::env::consts::ARCH)
            .expect("this platform is a release target")
    }

    fn unix(&self) -> impl Iterator<Item = &Target> {
        self.all.iter().filter(|t| t.os != "windows")
    }

    /// A SHA256SUMS for `version` listing every target's archive, `host` as the host's digest.
    fn sums(&self, version: &str, host: &str) -> String {
        self.all
            .iter()
            .enumerate()
            .map(|(i, target)| {
                let digest = if target.triple == self.host().triple {
                    host.to_string()
                } else {
                    format!("{i:064x}")
                };
                format!("{digest}  {}\n", self.archive(version, target))
            })
            .collect::<Vec<String>>()
            .concat()
    }
}

fn outcome(command: &mut Command) -> Outcome {
    let output = command.output().expect("command starts");
    Outcome {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

/// Runs `command` and panics with its output unless it succeeds.
fn succeed(command: &mut Command) -> Outcome {
    let outcome = outcome(command);
    assert_eq!(outcome.code, 0, "{command:?}:\n{}", outcome.stderr);
    outcome
}

fn sha256(path: &Path) -> String {
    let output = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .or_else(|_| Command::new("sha256sum").arg(path).output())
        .expect("shasum or sha256sum");
    let line = String::from_utf8(output.stdout).unwrap();
    line.split_whitespace().next().unwrap().to_string()
}

/// Every file under `dir`, relative to it, sorted.
fn files(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in fs::read_dir(&next).into_iter().flatten().flatten() {
            if entry.path().is_dir() {
                pending.push(entry.path());
            } else {
                let relative = entry.path().strip_prefix(dir).unwrap().to_owned();
                found.push(relative.display().to_string());
            }
        }
    }
    found.sort();
    found
}

/// The host's release archive, holding the `cupel` binary this test crate was built with.
fn host_archive() -> &'static (TempDir, PathBuf) {
    static ARCHIVE: OnceLock<(TempDir, PathBuf)> = OnceLock::new();
    ARCHIVE.get_or_init(|| {
        let targets = Targets::read();
        let host = targets.host();
        let stage = TempDir::new().unwrap();
        let dir = stage.path().join(targets.dir(host));
        fs::create_dir(&dir).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_cupel"), dir.join("cupel")).unwrap();
        let archive = stage.path().join(targets.archive(VERSION, host));
        succeed(
            Command::new("tar")
                .arg("-czf")
                .arg(&archive)
                .arg("-C")
                .arg(stage.path())
                .arg(targets.dir(host)),
        );
        (stage, archive)
    })
}

/// A release served over HTTP from a temporary directory, laid out as GitHub Releases serves
/// one: each file under both `download/v<version>/` and `latest/download/`.
struct Release {
    _root: TempDir,
    url: String,
}

impl Release {
    /// The host's archive with a SHA256SUMS that gives `digest` (its own when `None`).
    fn publish(digest: Option<&str>) -> Self {
        let targets = Targets::read();
        let (_, archive) = host_archive();
        let actual = sha256(archive);
        let sums = targets.sums(VERSION, digest.unwrap_or(&actual));
        let root = TempDir::new().unwrap();
        for prefix in [
            format!("download/v{VERSION}"),
            "latest/download".to_string(),
        ] {
            let dir = root.path().join(prefix);
            fs::create_dir_all(&dir).unwrap();
            fs::hard_link(archive, dir.join(archive.file_name().unwrap())).unwrap();
            fs::write(dir.join(&targets.checksums), &sums).unwrap();
        }
        let url = serve(root.path().to_path_buf());
        Self { _root: root, url }
    }
}

/// The release as published.
fn release() -> &'static Release {
    static RELEASE: OnceLock<Release> = OnceLock::new();
    RELEASE.get_or_init(|| Release::publish(None))
}

/// The digest `tampered()`'s SHA256SUMS gives the host's archive.
const WRONG_DIGEST: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

/// The release with a SHA256SUMS whose digest for the host's archive is wrong.
fn tampered() -> &'static Release {
    static RELEASE: OnceLock<Release> = OnceLock::new();
    RELEASE.get_or_init(|| Release::publish(Some(WRONG_DIGEST)))
}

/// Serves the files under `root` on a local port until the test process exits; its base URL.
fn serve(root: PathBuf) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let root = root.clone();
            std::thread::spawn(move || respond(&root, stream));
        }
    });
    url
}

/// Answers one GET with the file at its path under `root`, or 404.
fn respond(root: &Path, mut stream: TcpStream) {
    let mut request = String::new();
    let mut reader = BufReader::new(&stream);
    if reader.read_line(&mut request).is_err() {
        return;
    }
    let mut header = String::new();
    while reader.read_line(&mut header).is_ok_and(|read| read > 2) {
        header.clear();
    }
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let (status, body) = match fs::read(root.join(path.trim_start_matches('/'))) {
        Ok(body) => ("200 OK", body),
        Err(_) => ("404 Not Found", b"not found\n".to_vec()),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
}

/// What `binary --version` prints.
fn version_of(binary: &Path) -> String {
    succeed(Command::new(binary).arg("--version")).stdout
}

// cargo-binstall

/// The value binstall uses for `key` on `target`: its override, else the default.
fn binstall_value(binstall: &Item, target: &Target, key: &str) -> String {
    let value = binstall
        .get("overrides")
        .and_then(|overrides| overrides.get(target.triple.as_str()))
        .and_then(|table| table.get(key))
        .and_then(Item::as_str)
        .or_else(|| binstall[key].as_str())
        .unwrap_or_else(|| panic!("binstall `{key}` for {}", target.triple));
    value.to_string()
}

/// `value` with binstall's template variables filled in for `target`.
fn binstall_render(value: &str, repository: &str, target: &Target) -> String {
    value
        .replace("{ repo }", repository)
        .replace("{ version }", VERSION)
        .replace("{ target }", &target.triple)
        .replace("{ bin }", "cupel")
        .replace("{ binary-ext }", target.exe())
}

/// The repository URL the workspace manifest declares, which binstall's `{ repo }` names.
fn repository() -> String {
    let workspace = read_toml("Cargo.toml");
    workspace["workspace"]["package"]["repository"]
        .as_str()
        .unwrap()
        .to_string()
}

/// binstall's `pkg-url`, `bin-dir`, and `pkg-fmt` for `target`, rendered.
fn binstall_rendered(target: &Target, repository: &str) -> [String; 3] {
    let manifest = read_toml("crates/cupel-cli/Cargo.toml");
    let binstall = &manifest["package"]["metadata"]["binstall"];
    let render = |key| binstall_render(&binstall_value(binstall, target, key), repository, target);
    [render("pkg-url"), render("bin-dir"), render("pkg-fmt")]
}

/// Where dist/targets.toml says `target`'s archive is published, the binary's path in it, and
/// binstall's name for its format.
fn published(targets: &Targets, target: &Target, repository: &str) -> [String; 3] {
    let format = if target.format == "zip" { "zip" } else { "tgz" };
    [
        format!(
            "{repository}/releases/download/v{VERSION}/{}",
            targets.archive(VERSION, target)
        ),
        format!("{}/cupel{}", targets.dir(target), target.exe()),
        format.to_string(),
    ]
}

#[test]
fn binstall_metadata_names_each_targets_archive_and_binary() {
    let targets = Targets::read();
    let repository = repository();
    let rendered: Vec<[String; 3]> = targets
        .all
        .iter()
        .map(|target| binstall_rendered(target, &repository))
        .collect();
    let expected: Vec<[String; 3]> = targets
        .all
        .iter()
        .map(|target| published(&targets, target, &repository))
        .collect();
    assert_eq!(rendered, expected);
}

// install.sh

/// `sh dist/install.sh` against the release at `url`, with `HOME` at `home`.
fn install_script(url: &str, home: &Path) -> Command {
    let mut command = Command::new("sh");
    command
        .arg(workspace().join("dist/install.sh"))
        .env("HOME", home)
        .env("CUPEL_DOWNLOAD_URL", url)
        .env_remove("CUPEL_VERSION")
        .env_remove("CUPEL_INSTALL_DIR");
    command
}

/// `PATH` with `dir` in front.
fn path_with(dir: &Path) -> String {
    format!("{}:{}", dir.display(), std::env::var("PATH").unwrap())
}

#[test]
fn the_install_scripts_pass_shellcheck() {
    if !installed("shellcheck") {
        return;
    }
    let root = workspace();
    let outcome = outcome(
        Command::new("shellcheck")
            .args(["--shell=sh", "dist/install.sh", "dist/homebrew/render.sh"])
            .current_dir(root),
    );
    assert_eq!((outcome.code, outcome.stdout.as_str()), (0, ""));
}

#[test]
fn the_install_script_installs_the_latest_release_and_says_what_to_add_to_path() {
    let home = TempDir::new().unwrap();
    let dir = home.path().join("tools");
    let targets = Targets::read();
    let url = &release().url;
    let outcome = succeed(install_script(url, home.path()).env("CUPEL_INSTALL_DIR", &dir));
    assert_eq!(version_of(&dir.join("cupel")), format!("cupel {VERSION}\n"));
    let dir = dir.display();
    assert_eq!(
        outcome.stdout,
        format!(
            "cupel install: installed cupel {VERSION} as {dir}/cupel\n\
             Add {dir} to your PATH, for example in ~/.profile:\n  export PATH=\"{dir}:$PATH\"\n"
        )
    );
    assert_eq!(
        outcome.stderr,
        format!(
            "cupel install: downloading {url}/latest/download/{}\n",
            targets.archive(VERSION, targets.host())
        )
    );
}

#[test]
fn the_install_script_installs_into_local_bin_and_says_nothing_of_path_when_it_is_on_it() {
    let home = TempDir::new().unwrap();
    let dir = home.path().join(".local/bin");
    let outcome = succeed(install_script(&release().url, home.path()).env("PATH", path_with(&dir)));
    assert_eq!(version_of(&dir.join("cupel")), format!("cupel {VERSION}\n"));
    assert_eq!(
        outcome.stdout,
        format!(
            "cupel install: installed cupel {VERSION} as {}/cupel\n",
            dir.display()
        )
    );
}

#[test]
fn the_install_script_installs_the_version_cupel_version_names() {
    let home = TempDir::new().unwrap();
    let targets = Targets::read();
    let url = &release().url;
    let outcome =
        succeed(install_script(url, home.path()).env("CUPEL_VERSION", format!("v{VERSION}")));
    assert_eq!(
        outcome.stderr,
        format!(
            "cupel install: downloading {url}/download/v{VERSION}/{}\n",
            targets.archive(VERSION, targets.host())
        )
    );
    let missing = outcome_of_version(url, "9.9.9");
    assert_eq!(
        (missing.code, missing.stderr),
        (
            1,
            format!("cupel install: could not download {url}/download/v9.9.9/SHA256SUMS\n")
        )
    );
}

fn outcome_of_version(url: &str, version: &str) -> Outcome {
    let home = TempDir::new().unwrap();
    outcome(install_script(url, home.path()).env("CUPEL_VERSION", version))
}

#[test]
fn the_install_script_refuses_an_archive_whose_checksum_differs_and_installs_nothing() {
    let home = TempDir::new().unwrap();
    let targets = Targets::read();
    let archive = targets.archive(VERSION, targets.host());
    let url = &tampered().url;
    let outcome = outcome(&mut install_script(url, home.path()));
    let actual = sha256(&host_archive().1);
    assert_eq!(
        (outcome.code, outcome.stderr),
        (
            1,
            format!(
                "cupel install: downloading {url}/latest/download/{archive}\n\
                 cupel install: checksum mismatch for {archive}: SHA256SUMS has {WRONG_DIGEST}, the download is {actual}\n"
            )
        )
    );
    assert_eq!(files(home.path()), Vec::<String>::new());
}

/// A directory holding a `uname` that prints `system` and `machine`.
fn fake_uname(system: &str, machine: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    let uname = dir.path().join("uname");
    let script =
        format!("#!/bin/sh\ncase \"$1\" in -s) echo {system} ;; -m) echo {machine} ;; esac\n");
    fs::write(&uname, script).unwrap();
    fs::set_permissions(&uname, fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

/// install.sh's stderr on a machine whose `uname` prints `system` and `machine`.
fn install_on(system: &str, machine: &str) -> Outcome {
    let home = TempDir::new().unwrap();
    let uname = fake_uname(system, machine);
    outcome(
        install_script(&release().url, home.path())
            .env("CUPEL_INSTALL_DIR", home.path().join("bin"))
            .env("PATH", path_with(uname.path())),
    )
}

#[test]
fn the_install_script_downloads_each_unix_targets_archive() {
    let targets = Targets::read();
    let url = &release().url;
    let downloads: Vec<String> = targets.unix().map(first_download).collect();
    let expected: Vec<String> = targets
        .unix()
        .map(|target| {
            let archive = targets.archive(VERSION, target);
            format!("cupel install: downloading {url}/latest/download/{archive}")
        })
        .collect();
    assert_eq!(downloads, expected);
}

/// The first line install.sh writes to stderr on `target`, which names the download.
fn first_download(target: &Target) -> String {
    let (system, machine) = target.uname();
    let stderr = install_on(system, machine).stderr;
    stderr.lines().next().unwrap_or_default().to_string()
}

#[test]
fn the_install_script_names_a_platform_without_a_release() {
    let outcome = install_on("SunOS", "sparc");
    assert_eq!(
        (outcome.code, outcome.stderr),
        (
            1,
            "cupel install: no release for SunOS sparc; see https://github.com/tysont/cupel#install\n"
                .to_string()
        )
    );
}

// Homebrew

fn fixture_sums() -> PathBuf {
    workspace().join("crates/cupel-cli/tests/fixtures/SHA256SUMS")
}

fn render_formula(sums: &Path) -> Outcome {
    outcome(
        Command::new("sh")
            .arg(workspace().join("dist/homebrew/render.sh"))
            .arg(FIXTURE_VERSION)
            .arg(sums),
    )
}

/// Each `url` in `formula` with the `sha256` on the line after it, sorted.
fn urls_and_digests(formula: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = formula.lines().map(str::trim).collect();
    let quoted = |line: &str, keyword: &str| {
        let rest = line.strip_prefix(keyword)?.trim();
        Some(rest.strip_prefix('"')?.strip_suffix('"')?.to_string())
    };
    let mut pairs: Vec<(String, String)> = lines
        .windows(2)
        .filter_map(|pair| Some((quoted(pair[0], "url")?, quoted(pair[1], "sha256")?)))
        .collect();
    pairs.sort();
    pairs
}

#[test]
fn the_formula_renders_each_unix_archive_with_its_digest_from_sha256sums() {
    let targets = Targets::read();
    let outcome = render_formula(&fixture_sums());
    assert_eq!(outcome.code, 0, "{}", outcome.stderr);
    let sums: BTreeMap<String, String> = fs::read_to_string(fixture_sums())
        .unwrap()
        .lines()
        .map(|line| {
            let (digest, name) = line.split_once("  ").unwrap();
            (name.to_string(), digest.to_string())
        })
        .collect();
    let mut expected: Vec<(String, String)> = targets
        .unix()
        .map(|target| {
            let archive = targets.archive(FIXTURE_VERSION, target);
            let url = format!(
                "https://github.com/tysont/cupel/releases/download/v{FIXTURE_VERSION}/{archive}"
            );
            (url, sums[&archive].clone())
        })
        .collect();
    expected.sort();
    assert_eq!(urls_and_digests(&outcome.stdout), expected);
    let versions: Vec<&str> = outcome
        .stdout
        .lines()
        .filter(|line| line.contains('@') || line.trim().starts_with("version "))
        .collect();
    assert_eq!(versions, [format!("  version \"{FIXTURE_VERSION}\"")]);
}

#[test]
fn the_rendered_formula_is_ruby() {
    if !installed("ruby") {
        return;
    }
    let dir = TempDir::new().unwrap();
    let formula = dir.path().join("cupel.rb");
    let rendered = render_formula(&fixture_sums());
    assert_eq!(rendered.code, 0, "{}", rendered.stderr);
    fs::write(&formula, rendered.stdout).unwrap();
    let outcome = succeed(Command::new("ruby").arg("-c").arg(&formula));
    assert_eq!(outcome.stdout, "Syntax OK\n");
}

/// The fixture SHA256SUMS without `triple`'s line, written into `dir`; its path.
fn sums_without(dir: &Path, triple: &str) -> PathBuf {
    let sums = dir.join("SHA256SUMS");
    let all = fs::read_to_string(fixture_sums()).unwrap();
    let partial: String = all
        .lines()
        .filter(|line| !line.contains(triple))
        .flat_map(|line| [line, "\n"])
        .collect();
    fs::write(&sums, partial).unwrap();
    sums
}

#[test]
fn the_formula_is_not_rendered_from_sha256sums_missing_a_target() {
    let dir = TempDir::new().unwrap();
    let sums = sums_without(dir.path(), "aarch64-apple-darwin");
    let outcome = render_formula(&sums);
    assert_eq!(
        (outcome.code, outcome.stdout, outcome.stderr),
        (
            1,
            String::new(),
            format!(
                "render.sh: {} lists no cupel-{FIXTURE_VERSION}-aarch64-apple-darwin.tar.gz\n",
                sums.display()
            )
        )
    );
}

// The npm and pipx wrappers

/// The name Cupel's packages have on npm, the Python Package Index, and crates.io (ADR-146).
const PACKAGE: &str = "cupel-cli";

/// Each wrapper is the `cupel-cli` package and installs one command, `cupel`, from its one module.
#[test]
fn each_wrapper_is_the_cupel_cli_package_and_installs_the_cupel_command() {
    let npm: Value = serde_json::from_str(
        &fs::read_to_string(workspace().join("dist/npm/package.json")).unwrap(),
    )
    .unwrap();
    let pipx = read_toml("dist/pipx/pyproject.toml");
    let commands = |table: &Item| -> Vec<String> {
        table
            .as_table_like()
            .expect("a table of commands")
            .iter()
            .map(|(name, _)| name.to_owned())
            .collect()
    };
    let npm_commands: Vec<String> = npm["bin"]
        .as_object()
        .expect("a `bin` map")
        .keys()
        .cloned()
        .collect();
    let seen = (
        npm["name"].as_str().map(str::to_owned),
        npm_commands,
        pipx["project"]["name"].as_str().map(str::to_owned),
        commands(&pipx["project"]["scripts"]),
        pipx.get("tool")
            .and_then(|tool| tool.get("flit")?.get("module")?.get("name")?.as_str())
            .map(str::to_owned),
    );
    let expected = (
        Some(PACKAGE.to_owned()),
        vec!["cupel".to_owned()],
        Some(PACKAGE.to_owned()),
        vec!["cupel".to_owned()],
        Some("cupel".to_owned()),
    );
    assert_eq!(seen, expected);
}

/// A wrapper installed from its directory under dist/ into a temporary prefix, as its package
/// manager installs it from the registry.
struct Wrapper {
    dir: TempDir,
    /// The `cupel` command the package manager put on the prefix's `bin`.
    launcher: PathBuf,
    /// The environment the launcher needs, beyond the caller's.
    env: Vec<(String, PathBuf)>,
}

/// Whether this machine's `python3` imports `module`. Without it the test reports itself skipped,
/// unless `INSTALLER_TESTS_REQUIRE_TOOLS` is set, when it fails, as for a missing program.
fn python_module(module: &str) -> bool {
    let imports = Command::new("python3")
        .args(["-c", &format!("import {module}")])
        .output()
        .is_ok_and(|output| output.status.success());
    if imports {
        return true;
    }
    assert!(
        std::env::var_os("INSTALLER_TESTS_REQUIRE_TOOLS").is_none(),
        "python3 cannot import {module} and INSTALLER_TESTS_REQUIRE_TOOLS is set"
    );
    eprintln!("SKIPPED: python3 cannot import {module}; this test did not run");
    false
}

/// A copy of `dist/<name>` in `dir`, so packaging writes nothing into the repository.
fn copy_dist(name: &str, dir: &Path) -> PathBuf {
    succeed(
        Command::new("cp")
            .arg("-R")
            .arg(workspace().join("dist").join(name))
            .arg(dir),
    );
    dir.join(name)
}

impl Wrapper {
    /// dist/npm, packed and installed with `npm install --prefix`; `None` without node and npm.
    fn npm() -> Option<Self> {
        if !installed("node") || !installed("npm") {
            return None;
        }
        let dir = TempDir::new().unwrap();
        let source = copy_dist("npm", dir.path());
        let cache = dir.path().join("npm-cache");
        let npm = |args: &[&OsStr]| {
            succeed(
                Command::new("npm")
                    .args(args)
                    .env("npm_config_cache", &cache)
                    .current_dir(dir.path()),
            )
        };
        npm(&[
            "pack".as_ref(),
            "--pack-destination".as_ref(),
            dir.path().as_os_str(),
            source.as_os_str(),
        ]);
        let tarball = dir.path().join(format!("{PACKAGE}-{VERSION}.tgz"));
        let prefix = dir.path().join("prefix");
        npm(&[
            "install".as_ref(),
            "--offline".as_ref(),
            "--no-audit".as_ref(),
            "--no-fund".as_ref(),
            "--prefix".as_ref(),
            prefix.as_os_str(),
            tarball.as_os_str(),
        ]);
        let launcher = prefix.join("node_modules/.bin/cupel");
        Some(Self {
            dir,
            launcher,
            env: Vec::new(),
        })
    }

    /// dist/pipx, installed with `pip install --target`, built with the `flit_core` this machine's
    /// python3 has rather than one pip would fetch, so nothing here reaches the network (ADR-153);
    /// `None` without python3 or flit_core.
    fn pipx() -> Option<Self> {
        if !installed("python3") || !python_module("flit_core") {
            return None;
        }
        let dir = TempDir::new().unwrap();
        let source = copy_dist("pipx", dir.path());
        let site = dir.path().join("site");
        succeed(
            Command::new("python3")
                .args(["-m", "pip", "install", "--quiet", "--no-index"])
                .args(["--no-build-isolation", "--target"])
                .arg(&site)
                .arg(&source)
                .env("PIP_USER", "0")
                .env("PIP_DISABLE_PIP_VERSION_CHECK", "1")
                .env("PIP_NO_INPUT", "1"),
        );
        let launcher = site.join("bin/cupel");
        Some(Self {
            dir,
            launcher,
            env: vec![("PYTHONPATH".to_string(), site)],
        })
    }

    /// The launcher with the release at `url` and its cache in this wrapper's directory.
    fn command(&self, url: &str) -> assert_cmd::Command {
        let mut command = assert_cmd::Command::new(&self.launcher);
        for (key, _) in std::env::vars() {
            if key.starts_with("CUPEL_") {
                command.env_remove(key);
            }
        }
        command
            .env("CUPEL_DOWNLOAD_URL", url)
            .env("CUPEL_CACHE_DIR", self.cache())
            .envs(self.env.iter().map(|(key, value)| (key, value)));
        command
    }

    fn cache(&self) -> PathBuf {
        self.dir.path().join("cache")
    }
}

fn run(command: &mut assert_cmd::Command, args: &[&str]) -> Outcome {
    let output = command.args(args).output().expect("launcher starts");
    Outcome {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

/// The host's archive and where the wrappers cache its binary.
fn host_download() -> (String, String) {
    let targets = Targets::read();
    let host = targets.host();
    (
        targets.archive(VERSION, host),
        format!("{VERSION}/{}/cupel", host.triple),
    )
}

/// One run's exit code, stdout, and stderr.
type Ran = (i32, String, String);

fn ran(outcome: Outcome) -> Ran {
    (outcome.code, outcome.stdout, outcome.stderr)
}

/// Two `cupel --version` runs through `wrapper` with an empty cache: each run, and the files
/// cached after the first.
fn version_twice(wrapper: &Wrapper) -> (Ran, Vec<String>, Ran) {
    let url = &release().url;
    let first = ran(run(&mut wrapper.command(url), &["--version"]));
    let cached = files(&wrapper.cache());
    let second = ran(run(&mut wrapper.command(url), &["--version"]));
    (first, cached, second)
}

/// What `version_twice` sees: one download, then the cached binary alone.
fn downloaded_once() -> (Ran, Vec<String>, Ran) {
    let url = &release().url;
    let (archive, cached) = host_download();
    let version = format!("cupel {VERSION}\n");
    (
        (
            0,
            version.clone(),
            format!("cupel: downloading {url}/download/v{VERSION}/{archive}\n"),
        ),
        vec![cached],
        (0, version, String::new()),
    )
}

/// A repository whose tree gate fails: one function of seven parameters against `< 5`.
fn failing_gate() -> Repo {
    let repo = Repo::new();
    repo.write(
        "src/wide.rs",
        "pub fn wide(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) -> i32 {\n    a + b + c + d + e + f + g\n}\n",
    );
    repo.write(
        "cupel.toml",
        "depth = \"quick\"\n[gate.tree]\n\"static.complexity.params_max\" = \"< 5\"\n",
    );
    repo
}

/// `cupel check` on `failing_gate()` through `wrapper`: its exit code and the envelope's gate result.
fn failing_check(wrapper: &Wrapper) -> (i32, Value) {
    let repo = failing_gate();
    let outcome = repo.run_with(&mut wrapper.command(&release().url), &["check"]);
    let envelope = outcome.envelope();
    (outcome.code, envelope["summary"]["gate"]["result"].clone())
}

/// `cupel --version` through `wrapper` against the tampered release, and the files it cached.
fn tampered_version(wrapper: &Wrapper) -> (Ran, Vec<String>) {
    let outcome = run(&mut wrapper.command(&tampered().url), &["--version"]);
    (ran(outcome), files(&wrapper.cache()))
}

/// What `tampered_version` sees: exit 3, the mismatch named, and nothing cached.
fn refused() -> (Ran, Vec<String>) {
    let url = &tampered().url;
    let (archive, _) = host_download();
    let actual = sha256(&host_archive().1);
    let stderr = format!(
        "cupel: downloading {url}/download/v{VERSION}/{archive}\n\
         cupel: checksum mismatch for {archive}: SHA256SUMS has {WRONG_DIGEST}, the download is {actual}\n"
    );
    ((3, String::new(), stderr), Vec::new())
}

/// The version and target table every wrapper must carry: Cupel's, and dist/targets.toml's.
fn expected_release() -> Value {
    let targets = Targets::read();
    let table: serde_json::Map<String, Value> = targets
        .all
        .iter()
        .map(|target| (target.key(), json!([target.triple, target.format])))
        .collect();
    json!({ "version": VERSION, "targets": table })
}

/// The wrapper module `path` under dist/ loaded by `program` with `args` and `script`, which
/// prints its version and target table as JSON; `None` without `program`.
fn wrapper_release(program: &str, args: &[&str], script: &str, path: &str) -> Option<Value> {
    if !installed(program) {
        return None;
    }
    let outcome = succeed(
        Command::new(program)
            .args(args)
            .arg(script)
            .arg(workspace().join(path)),
    );
    let release = serde_json::from_str(&outcome.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}", outcome.stdout));
    Some(release)
}

/// The npm and pipx wrappers, each installed; one whose tools are missing is left out.
fn wrappers() -> Vec<(&'static str, Wrapper)> {
    [("npm", Wrapper::npm()), ("pipx", Wrapper::pipx())]
        .into_iter()
        .filter_map(|(name, wrapper)| Some((name, wrapper?)))
        .collect()
}

/// Through each wrapper, with its cache empty: the tampered release is refused and caches
/// nothing; the release is then downloaded once and run; and a failing gate exits 1.
#[test]
fn each_wrapper_refuses_a_bad_checksum_downloads_once_and_forwards_the_exit_code() {
    let wrappers = wrappers();
    let seen: Vec<_> = wrappers
        .iter()
        .map(|(name, wrapper)| {
            let refusal = tampered_version(wrapper);
            let runs = version_twice(wrapper);
            (*name, refusal, runs, failing_check(wrapper))
        })
        .collect();
    let expected: Vec<_> = wrappers
        .iter()
        .map(|(name, _)| (*name, refused(), downloaded_once(), (1, json!("fail"))))
        .collect();
    assert_eq!(seen, expected);
}

#[test]
fn the_npm_wrapper_fetches_cupels_version_for_each_release_target() {
    let script = "const m = require(process.argv[1]); console.log(JSON.stringify({ version: m.VERSION, targets: m.TARGETS }));";
    let Some(release) = wrapper_release("node", &["-e"], script, "dist/npm/bin/cupel.js") else {
        return;
    };
    assert_eq!(release, expected_release());
}

#[test]
fn the_pipx_wrapper_fetches_cupels_version_for_each_release_target() {
    // `-B`: importing the module from dist/ writes no bytecode into the repository.
    let script = "import json, sys; sys.path.insert(0, sys.argv[1]); import cupel; print(json.dumps({'version': cupel.__version__, 'targets': cupel.TARGETS}))";
    let Some(release) = wrapper_release("python3", &["-B", "-c"], script, "dist/pipx") else {
        return;
    };
    assert_eq!(release, expected_release());
}
