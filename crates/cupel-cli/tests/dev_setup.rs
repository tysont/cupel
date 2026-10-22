// ABOUTME: Tests for the development tool environment (ADR-139): every version ci.yml, its tools action, and release.yml's build job pin
// ABOUTME: appears in scripts/dev-setup.sh, the scripts pass ShellCheck, and `make dev-env` exports only tools that exist.
mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use support::{installed, jobs, workflow, workspace};
use tempfile::TempDir;

fn dev_setup_script() -> String {
    fs::read_to_string(workspace().join("scripts/dev-setup.sh")).unwrap()
}

/// Whether `text` looks like a version: it starts with a digit, or is a dated toolchain.
fn is_version(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_digit()) || text.starts_with("nightly-")
}

/// The version segments of a URL's path: `v1.7.4` and `3.9.16`, not `maven-3`.
fn url_versions(url: &str) -> impl Iterator<Item = &str> {
    url.split('/')
        .skip(1)
        .map(|segment| segment.strip_prefix('v').unwrap_or(segment))
        .filter(|segment| {
            segment.split('.').count() > 1
                && segment.split('.').all(|part| part.parse::<u64>().is_ok())
        })
}

/// The versions `words[i]` names: a URL's version segments, `pkg==X`, `pkg@X`, or the word itself
/// after `--version`, a `<tool>-version:` key, or `toolchain install`.
fn word_versions<'a>(words: &[&'a str], i: usize) -> Vec<&'a str> {
    let word = words[i];
    let previous = |back: usize| i.checked_sub(back).map(|j| words[j]);
    if let Some(at) = word.find("://") {
        return url_versions(&word[at + 3..]).collect();
    }
    let named = word
        .split_once("==")
        .or_else(|| word.rsplit_once('@'))
        .map(|(_, version)| version);
    let after_key = previous(1) == Some("--version")
        || previous(1).is_some_and(|key| key.ends_with("-version:"))
        || (previous(2), previous(1)) == (Some("toolchain"), Some("install"));
    named
        .or(after_key.then_some(word))
        .into_iter()
        .filter(|version| is_version(version))
        .collect()
}

/// The versions `lines` of a workflow pin, each with the line it is on: `pkg==X`, `pkg@X`,
/// `--version X`, `<tool>-version: 'X'`, `toolchain install X`, and version segments of URL
/// paths. Comments and `uses:` lines (action references, pinned by release.rs) are not read.
fn pins<'a>(lines: impl IntoIterator<Item = &'a str>) -> Vec<(String, String)> {
    let mut pins = Vec::new();
    for line in lines.into_iter().map(str::trim) {
        let body = line.strip_prefix("- ").unwrap_or(line);
        if body.starts_with('#') || body.starts_with("uses:") {
            continue;
        }
        let words: Vec<&str> = body
            .split_whitespace()
            .map(|word| word.trim_matches(|c| c == '"' || c == '\''))
            .collect();
        pins.extend(
            (0..words.len())
                .flat_map(|i| word_versions(&words, i))
                .map(|version| (line.to_owned(), version.to_owned())),
        );
    }
    pins
}

/// The steps of a job, from its trimmed lines: each starts at a `- ` list item.
fn steps(lines: &[String]) -> Vec<&[String]> {
    let starts: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].starts_with("- "))
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &start)| &lines[start..starts.get(n + 1).copied().unwrap_or(lines.len())])
        .collect()
}

/// The composite action that installs the tools ci.yml's `check` and `dogfood` jobs share.
fn tools_action() -> String {
    fs::read_to_string(workspace().join(".github/actions/tools/action.yml")).unwrap()
}

/// Every pin the development environment mirrors: ci.yml's jobs but `windows` and its tools action, and the steps of
/// release.yml's `build` job that run on Linux or macOS (`make dist` is the local route to them). The Windows-only
/// job and step (`choco install make`) and the publish jobs, which only CI runs, are not mirrored.
fn workflow_pins() -> Vec<(String, String)> {
    let ci = jobs(&workflow("ci.yml"));
    let mut all: Vec<(String, String)> = ci
        .iter()
        .filter(|(name, _)| name != "windows")
        .flat_map(|(_, lines)| pins(lines.iter().map(String::as_str)))
        .collect();
    all.extend(pins(tools_action().lines()));
    let release = jobs(&workflow("release.yml"));
    let (_, build) = release
        .iter()
        .find(|(name, _)| name == "build")
        .expect("a `build` job");
    for step in steps(build) {
        if !step.iter().any(|line| line == "if: runner.os == 'Windows'") {
            all.extend(pins(step.iter().map(String::as_str)));
        }
    }
    all
}

#[test]
fn pins_reads_each_way_a_workflow_writes_a_version() {
    let yaml = "\
# Pinned: cargo install ignored-in-comments --version 9.9.9
- uses: actions/setup-go@v5
  with:
    go-version: '1.23'
    tool: cargo-nextest@0.9.148
run: |
  python -m pip install hypothesis==6.170.1 pytest
  npm install --global fast-check@4.10.2 @jazzer.js/core@4.0.0 vitest
  cargo install cargo-fuzz --version 0.13.2 --locked
  rustup toolchain install nightly-2026-03-26 --profile minimal
  rustup toolchain install
  curl -sSfL https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar
  curl -sSfL https://archive.apache.org/dist/maven/maven-3/3.9.16/binaries/apache-maven-3.9.16-bin.tar.gz | tar xz
  echo \"$RUNNER_TEMP/zig-x86_64-linux-0.16.0\" >> \"$GITHUB_PATH\"
node-version: \"22.23.3\"
";
    let versions: Vec<String> = pins(yaml.lines()).into_iter().map(|(_, v)| v).collect();
    assert_eq!(
        versions,
        vec![
            "1.23",
            "0.9.148",
            "6.170.1",
            "4.10.2",
            "4.0.0",
            "0.13.2",
            "nightly-2026-03-26",
            "1.7.4",
            "3.9.16",
            "22.23.3",
        ]
    );
}

/// A version pinned in ci.yml, its tools action, or release.yml's build job and not in scripts/dev-setup.sh means the
/// local environment and CI have drifted: goldens regenerated locally would not match CI's.
#[test]
fn dev_setup_installs_every_version_the_workflows_pin() {
    let script = dev_setup_script();
    let pins = workflow_pins();
    assert!(pins.len() > 20, "too few pins found: {pins:?}");
    let missing: Vec<&(String, String)> = pins
        .iter()
        .filter(|(_, version)| !script.contains(version.as_str()))
        .collect();
    assert_eq!(missing, Vec::<&(String, String)>::new());
}

/// Whether a package argument of `pip install` or `npm install` names its version.
fn names_a_version(package: &str) -> bool {
    package.contains("==")
        || package
            .rsplit_once('@')
            .is_some_and(|(_, version)| is_version(version))
}

/// Whether a ci.yml line that installs by name pins what it installs: a `tool:` entry with `@X`,
/// a Node or Python version with its patch release, and every package of `pip install` and
/// `npm install` (flags and quoted paths are not packages). Other lines pin nothing to check.
fn pins_what_it_installs(line: &str) -> bool {
    let command = line.strip_prefix("run: ").unwrap_or(line);
    if let Some(tool) = command.strip_prefix("tool: ") {
        return tool.contains('@');
    }
    if let Some(version) = command
        .strip_prefix("node-version: ")
        .or_else(|| command.strip_prefix("python-version: "))
    {
        return version.trim_matches('"').split('.').count() == 3;
    }
    if !(command.contains("pip install ") || command.contains("npm install ")) {
        return true;
    }
    command
        .split_whitespace()
        .skip_while(|word| *word != "install")
        .skip(1)
        .filter(|word| !word.starts_with(['-', '"']))
        .all(names_a_version)
}

/// ci.yml and its tools action name a version for every tool they install by name and a patch release of Node and
/// Python, so a local install can match it. Go's `1.23` is its final minor release and the JDK's
/// `21` resolves to one release, which scripts/dev-setup.sh names exactly.
#[test]
fn ci_pins_every_package_and_runtime_it_installs() {
    let ci = workflow("ci.yml") + &tools_action();
    let unpinned: Vec<&str> = ci
        .lines()
        .map(str::trim)
        .filter(|line| !pins_what_it_installs(line))
        .collect();
    assert_eq!(unpinned, Vec::<&str>::new());
}

#[test]
fn the_dev_scripts_pass_shellcheck() {
    if !installed("shellcheck") {
        return;
    }
    let output = Command::new("shellcheck")
        .args(["--shell=sh", "scripts/dev-setup.sh", "scripts/dev-env.sh"])
        .current_dir(workspace())
        .output()
        .unwrap();
    assert_eq!(
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned()
        ),
        (Some(0), String::new())
    );
}

/// `make dev-env` with the tools directory at `tools`: the `export` lines it prints.
fn dev_env(tools: &Path) -> Vec<String> {
    let mut command = Command::new("make");
    for variable in ["MAKEFLAGS", "MFLAGS", "MAKELEVEL"] {
        command.env_remove(variable);
    }
    let output = command
        .args(["--no-print-directory", "-s", "dev-env"])
        .arg(format!("CUPEL_DEV_TOOLS={}", tools.display()))
        .current_dir(workspace())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Without a tools directory, as on a CI runner, the Makefile changes no variable: the runner's
/// own `JAVA_HOME` and `PATH` stand.
#[test]
fn dev_env_exports_nothing_without_the_tools_directory() {
    let dir = TempDir::new().unwrap();
    assert_eq!(dev_env(&dir.path().join("absent")), Vec::<String>::new());
}

/// With a tools directory, PATH gains the directory of each tool that is there, in front, and each
/// tool's home and caches point into it.
#[test]
fn dev_env_puts_the_installed_tools_first_on_path() {
    let dir = TempDir::new().unwrap();
    let tools = dir.path().canonicalize().unwrap();
    for sub in ["bin", "node/bin", "jdk/bin"] {
        fs::create_dir_all(tools.join(sub)).unwrap();
    }
    let exports = dev_env(&tools);
    let t = tools.display();
    let path = exports
        .iter()
        .find_map(|line| line.strip_prefix("export PATH='"))
        .expect("a PATH export");
    assert!(
        path.starts_with(&format!("{t}/bin:{t}/node/bin:{t}/jdk/bin:")),
        "{path}"
    );
    let others: Vec<&String> = exports
        .iter()
        .filter(|line| !line.starts_with("export PATH="))
        .collect();
    assert_eq!(
        others,
        vec![
            &format!("export JAVA_HOME='{t}/jdk'"),
            &format!("export MAVEN_ARGS='-Dmaven.repo.local={t}/m2/repository'"),
            &format!("export GRADLE_USER_HOME='{t}/gradle-home'"),
            &format!("export CLJ_CONFIG='{t}/clojure-config'"),
            &format!("export CLJ_CACHE='{t}/clojure-cache'"),
            &format!("export KANI_HOME='{t}/kani-home'"),
        ]
    );
}
