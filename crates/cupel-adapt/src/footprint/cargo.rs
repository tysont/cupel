// ABOUTME: The Rust ecosystem of `tool.footprint` (ADR-103): every package of `Cargo.lock` except the workspace's own,
// ABOUTME: with licenses and the workspace members from `cargo metadata --offline --locked` when it succeeds.
use std::collections::{BTreeMap, BTreeSet};

use cupel_core::{Dependency, Process, find_program};
use serde_json::Value;

use super::{Context, Ecosystem, Project, Root, line_after};

pub static CARGO: Ecosystem = Ecosystem {
    langs: &["rust"],
    osv: "crates.io",
    discover,
    parse,
    normalize: str::to_owned,
};

const LOCKFILE: &str = "Cargo.lock";

/// `Cargo.lock` when the root has one; the workspace's members and every package's license
/// from `cargo metadata`, else the lockfile's packages with no source as members and no
/// licenses.
fn discover(root: &Root<'_>) -> Project {
    if !root.path.join(LOCKFILE).is_file() {
        return Project::default();
    }
    let context = match metadata(root) {
        Some(metadata) => Context {
            members: metadata.members,
            licenses: Some(metadata.licenses),
            ..Context::default()
        },
        None => Context {
            members: std::fs::read_to_string(root.path.join(LOCKFILE))
                .map(|text| sourceless(&text))
                .unwrap_or_default(),
            ..Context::default()
        },
    };
    Project {
        manifests: vec![root.file(LOCKFILE)],
        context,
    }
}

/// The names of the lockfile's packages that have no source (path packages).
fn sourceless(text: &str) -> BTreeSet<String> {
    packages(text)
        .into_iter()
        .filter(|package| package.get("source").is_none())
        .filter_map(|package| Some(package.get("name")?.as_str()?.to_owned()))
        .collect()
}

fn packages(text: &str) -> Vec<toml::Table> {
    let Ok(lock) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    lock.get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|package| package.as_table().cloned())
        .collect()
}

/// Every locked package that is not a member; direct when a member's `dependencies` name it.
fn parse(file: &str, text: &str, context: &Context) -> Vec<Dependency> {
    let packages = packages(text);
    let named = |package: &toml::Table, key: &str| {
        package
            .get(key)
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let direct: BTreeSet<String> = packages
        .iter()
        .filter(|package| context.members.contains(&named(package, "name")))
        .flat_map(|package| {
            package
                .get("dependencies")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|entry| entry.as_str()?.split(' ').next().map(str::to_owned))
        })
        .collect();
    let mut from = 0;
    let mut found = Vec::new();
    for package in &packages {
        let name = named(package, "name");
        let (line, next) = line_after(text, from, |line| line == format!("name = \"{name}\""));
        from = next;
        if context.members.contains(&name) {
            continue;
        }
        let version = Some(named(package, "version")).filter(|version| !version.is_empty());
        found.push(Dependency {
            direct: direct.contains(&name),
            license: context.license(&name, version.as_deref()),
            version,
            name,
            file: file.to_owned(),
            line,
        });
    }
    found
}

/// What `cargo metadata` says about the workspace.
struct Metadata {
    /// The members' names.
    members: BTreeSet<String>,
    /// Every other package's license, by `name@version`.
    licenses: BTreeMap<String, String>,
}

/// `cargo metadata --format-version 1 --offline --locked` in the root: it writes nothing and
/// fetches nothing, and fails rather than resolve; `None` when it fails, is cancelled (which the
/// producer reports at its next checkpoint), or cargo is missing.
fn metadata(root: &Root<'_>) -> Option<Metadata> {
    find_program("cargo")?;
    let captured = Process::new("cargo", &root.path)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--offline",
            "--locked",
            "--quiet",
        ])
        .env("CARGO_TERM_COLOR", "never")
        .run(root.cancel)
        .ok()
        .filter(|captured| captured.exit_code == Some(0))?;
    let value = serde_json::from_str::<Value>(&captured.stdout).ok()?;
    let members: BTreeSet<&str> = value["workspace_members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut metadata = Metadata {
        members: BTreeSet::new(),
        licenses: BTreeMap::new(),
    };
    for package in value["packages"].as_array().into_iter().flatten() {
        let text = |key: &str| package[key].as_str().unwrap_or_default().to_owned();
        if members.contains(package["id"].as_str().unwrap_or_default()) {
            metadata.members.insert(text("name"));
        } else if let Some(license) = package["license"].as_str() {
            metadata.licenses.insert(
                format!("{}@{}", text("name"), text("version")),
                license.to_owned(),
            );
        }
    }
    Some(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"# This file is automatically @generated by Cargo.
version = 4

[[package]]
name = "ledger"
version = "0.1.0"
dependencies = [
 "itoa",
 "ryu 1.0.0",
]

[[package]]
name = "itoa"
version = "1.0.11"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "ryu"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = [
 "itoa",
]
"#;

    #[test]
    fn members_are_left_out_and_their_dependencies_are_direct() {
        let context = Context {
            members: sourceless(LOCK),
            ..Context::default()
        };
        assert_eq!(context.members, BTreeSet::from(["ledger".to_owned()]));
        let found: Vec<(String, Option<String>, bool, u32)> = parse("Cargo.lock", LOCK, &context)
            .into_iter()
            .map(|d| (d.name, d.version, d.direct, d.line))
            .collect();
        assert_eq!(
            found,
            [
                ("itoa".to_owned(), Some("1.0.11".to_owned()), true, 13),
                ("ryu".to_owned(), Some("1.0.0".to_owned()), true, 18),
            ]
        );
    }

    #[test]
    fn an_unreadable_lockfile_lists_nothing() {
        assert_eq!(parse("Cargo.lock", "[[package", &Context::default()), []);
    }
}
