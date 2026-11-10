// ABOUTME: pnpm's `pnpm-lock.yaml` (lockfile v9) for the npm ecosystem of `tool.footprint`: what one importer (a project
// ABOUTME: root of the workspace) locks, through the snapshots its packages need, with licenses from `node_modules/.pnpm`.
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use cupel_core::Dependency;

use super::{Context, Root};

pub const LOCKFILE: &str = "pnpm-lock.yaml";

/// The groups of an importer or a snapshot whose packages are installed.
const GROUPS: [&str; 3] = ["dependencies", "devDependencies", "optionalDependencies"];

/// The workspace file whose directory's lockfile its members are installed from.
const WORKSPACE: &str = "pnpm-workspace.yaml";

/// The lockfile pnpm installs a project root from, and the root's importer in it.
pub struct Locking {
    /// Relative to the repository root.
    pub file: String,
    pub importer: String,
    /// The directory holding the lockfile, absolute: where pnpm installs packages.
    pub dir: PathBuf,
}

/// The lockfile of the nearest workspace at or above `root` in the repository (a member's own
/// lockfile is not what pnpm installs it from), else of `root` itself, when it has an importer
/// for `root`.
pub fn locking(root: &Root<'_>) -> Option<Locking> {
    let depth = Path::new(root.dir)
        .components()
        .filter(|part| matches!(part, std::path::Component::Normal(_)))
        .count();
    let repository = root.path.ancestors().nth(depth)?;
    let dir = Path::new(root.dir)
        .ancestors()
        .map(|dir| dir.to_str().filter(|dir| !dir.is_empty()).unwrap_or("."))
        .find(|dir| repository.join(dir).join(WORKSPACE).is_file())
        .unwrap_or(root.dir);
    let importer = match dir {
        "." => root.dir.to_owned(),
        _ if dir == root.dir => ".".to_owned(),
        _ => root.dir[dir.len() + 1..].to_owned(),
    };
    let text = std::fs::read_to_string(repository.join(dir).join(LOCKFILE)).ok()?;
    Lock::read(&text)
        .importers
        .contains_key(&importer)
        .then(|| Locking {
            file: crate::tree::under(dir, LOCKFILE),
            importer,
            dir: repository.join(dir),
        })
}

/// The license of each package installed under `node_modules/.pnpm` in `dir`, by `name@version`;
/// `None` when nothing is installed there.
pub fn installed_licenses(dir: &Path) -> Option<BTreeMap<String, String>> {
    let store = std::fs::read_dir(dir.join("node_modules/.pnpm")).ok()?;
    let mut licenses = BTreeMap::new();
    for entry in store.flatten() {
        let entry_name = entry.file_name();
        let Some((name, _)) = entry_name.to_str().and_then(split_key) else {
            continue;
        };
        let manifest = entry
            .path()
            .join("node_modules")
            .join(name.replacen('+', "/", 1))
            .join("package.json");
        let Ok(text) = std::fs::read_to_string(manifest) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let license = manifest["license"]
            .as_str()
            .or_else(|| manifest["license"]["type"].as_str());
        if let (Some(name), Some(version), Some(license)) = (
            manifest["name"].as_str(),
            manifest["version"].as_str(),
            license,
        ) {
            licenses.insert(format!("{name}@{version}"), license.to_owned());
        }
    }
    Some(licenses)
}

/// What `context.importer` locks in `text`, the contents of `file`: its own packages, direct,
/// then those they need, each once and at its line under `packages`. Workspace links are the
/// project's own and are left out.
pub fn parse(file: &str, text: &str, context: &Context) -> Vec<Dependency> {
    let Some(importer) = context.importer.as_deref() else {
        return Vec::new();
    };
    let lock = Lock::read(text);
    let mut queue: VecDeque<(String, String, bool)> = lock
        .importers
        .get(importer)
        .into_iter()
        .flatten()
        .map(|(name, version)| (name.clone(), version.clone(), true))
        .collect();
    let mut visited = BTreeSet::new();
    let mut found: Vec<Dependency> = Vec::new();
    while let Some((name, version, direct)) = queue.pop_front() {
        if version.starts_with("link:") || !visited.insert(format!("{name}@{version}")) {
            continue;
        }
        let resolved = version.split('(').next().unwrap_or_default().to_owned();
        let key = format!("{name}@{resolved}");
        if !found
            .iter()
            .any(|known| known.name == name && known.version.as_deref() == Some(&resolved))
        {
            found.push(Dependency {
                license: context.license(&name, Some(&resolved)),
                line: lock.lines.get(&key).copied().unwrap_or(1),
                name: name.clone(),
                version: Some(resolved),
                direct,
                file: file.to_owned(),
            });
        }
        for (needed, at) in lock
            .snapshots
            .get(&format!("{name}@{version}"))
            .into_iter()
            .flatten()
        {
            queue.push_back((needed.clone(), at.clone(), false));
        }
    }
    found
}

/// The parts of a lockfile the footprint reads.
#[derive(Default)]
struct Lock {
    /// Each importer's packages, `(name, version)`, the version with its peers as snapshots name
    /// it; an importer that needs nothing is listed with none.
    importers: BTreeMap<String, Vec<(String, String)>>,
    /// The packages each snapshot needs, by its key.
    snapshots: BTreeMap<String, Vec<(String, String)>>,
    /// The 1-based line of each `name@version` under `packages`.
    lines: BTreeMap<String, u32>,
}

impl Lock {
    /// Reads the block layout pnpm writes: top-level sections, two spaces per level.
    fn read(text: &str) -> Self {
        let mut lock = Self::default();
        let (mut section, mut entry, mut group, mut name) = ("", String::new(), "", String::new());
        for (index, line) in text.lines().enumerate() {
            let content = line.trim_start();
            if content.is_empty() {
                continue;
            }
            let (key, value) = match content.split_once(": ") {
                Some((key, value)) => (unquote(key), value.trim()),
                None => (unquote(content.trim_end_matches(':')), ""),
            };
            match (line.len() - content.len(), section) {
                (0, _) => {
                    section = if value.is_empty() {
                        section_named(&key)
                    } else {
                        ""
                    }
                }
                (2, "importers") => {
                    lock.importers.entry(key.clone()).or_default();
                    entry = key;
                }
                (2, "snapshots") => entry = key,
                (2, "packages") => {
                    lock.lines
                        .insert(key, u32::try_from(index + 1).unwrap_or(u32::MAX));
                }
                (4, "importers" | "snapshots") => {
                    group = GROUPS.iter().find(|g| **g == key).copied().unwrap_or("");
                }
                (6, "importers") if !group.is_empty() => name = key,
                (6, "snapshots") if !group.is_empty() => lock
                    .snapshots
                    .entry(entry.clone())
                    .or_default()
                    .push((key, unquote(value))),
                (8, "importers") if !group.is_empty() && key == "version" => lock
                    .importers
                    .entry(entry.clone())
                    .or_default()
                    .push((name.clone(), unquote(value))),
                _ => {}
            }
        }
        lock
    }
}

/// The section the reader follows that `key` names, else none.
fn section_named(key: &str) -> &'static str {
    ["importers", "packages", "snapshots"]
        .into_iter()
        .find(|section| *section == key)
        .unwrap_or("")
}

fn unquote(text: &str) -> String {
    let text = text.trim();
    text.strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .or_else(|| {
            text.strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
        })
        .unwrap_or(text)
        .to_owned()
}

/// `name@version…` split at the `@` that ends the name (a scoped name starts with one).
fn split_key(key: &str) -> Option<(&str, &str)> {
    let at = key.get(1..)?.find('@')? + 1;
    Some((&key[..at], &key[at + 1..]))
}
