// ABOUTME: A JavaScript or TypeScript package's entries (ADR-176, ADR-192): the files its package.json `exports` map,
// ABOUTME: or `main` and `types`, names, and what they re-export; an exported item they do not reach is internal.
//!
//! - **The package** of a file is the nearest directory above it with a `package.json`.
//! - **Entries.** Every target of `exports` (a string, an array, subpaths, and conditions, all of
//!   them, nested); without `exports`, `main`, `types`, and `typings`. A manifest with none of
//!   them, or no manifest, has no entries, so every item of the package is internal.
//! - **Targets** resolve as imports do (the file, its TypeScript source, an extension, a
//!   directory's index). A target that names no file and whose path is built output
//!   (`./dist/index.js`, `./dist/esm/index.d.ts`) stands for the source of the same path under
//!   `src`, its leading directories dropped one by one until a file is found (`src/index.ts`).
//!   No other mapping is made: a target whose source is elsewhere exposes nothing.
//! - **Wildcards.** A target with `*` (`"./*": "./src/*.ts"`) exposes every file of the
//!   language that it matches, `*` matching across directories; a built one is matched under
//!   `src` as above, by path without extension. A `null` target (an exclusion) is ignored.
//! - **Re-exports** (`export … from`, `export *`) of an exposed file expose what they name of
//!   their target, followed transitively: an entry exposes every name it exports, `export *`
//!   passes on what its file exposes, and `export { a as b }` exposes `a` when `b` is exposed.
use std::collections::{BTreeMap, BTreeSet, HashMap};

use cupel_core::syntax::{ApiItem, ApiSurface, EdgeKind, ImportTarget};
use serde_json::Value;

use super::resolve::{Package, Resolver, match_pattern, normalize};
use crate::imports::{Imports, RawImport, join};
use crate::namespace::file_namespace;
use crate::tables::NamespaceRules;

/// The extensions of built output a manifest may name in place of a source's, longest first.
const BUILT: &[&str] = &[".d.ts", ".d.mts", ".d.cts", ".js", ".mjs", ".cjs"];

/// The directory of a package's sources, which its built output stands for.
const SOURCES: &str = "src";

/// What of a file its package's entries expose: every name it exports, or only these.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Names {
    All,
    Only(BTreeSet<String>),
}

impl Names {
    /// Whether an item whose top-level name is `name` is exposed; an item whose name is not
    /// known is exposed only when every name is.
    fn allows(&self, name: Option<&str>) -> bool {
        match (self, name) {
            (Self::All, _) => true,
            (Self::Only(names), Some(name)) => names.contains(name),
            (Self::Only(_), None) => false,
        }
    }

    /// Grows `self` by `other`; whether it grew.
    fn absorb(&mut self, other: Self) -> bool {
        match (&mut *self, other) {
            (Self::All, _) => false,
            (_, Self::All) => {
                *self = Self::All;
                true
            }
            (Self::Only(names), Self::Only(more)) => {
                let before = names.len();
                names.extend(more);
                names.len() > before
            }
        }
    }
}

/// Marks each item of `surface` that the entries of its package do not reach (ADR-176).
pub(super) fn mark_internal(
    surface: &mut ApiSurface,
    files: &Imports<'_>,
    typescript: bool,
    rules: &NamespaceRules,
) {
    let resolver = Resolver::new(files, typescript);
    let packages = resolver.packages();
    let mut reached: HashMap<Option<String>, BTreeMap<String, Names>> = HashMap::new();
    for item in &mut surface.items {
        let dir = resolver.package_dir_of(&item.file);
        let exposed = reached.entry(dir.clone()).or_insert_with(|| {
            dir.map(|dir| reach(&resolver, &packages, files, &dir))
                .unwrap_or_default()
        });
        item.internal = !exposed
            .get(&item.file)
            .is_some_and(|names| names.allows(top_name(item, rules)));
    }
}

/// What the entries of the package in `dir` expose, by file.
fn reach(
    resolver: &Resolver<'_, '_>,
    packages: &[Package],
    files: &Imports<'_>,
    dir: &str,
) -> BTreeMap<String, Names> {
    let mut reached: BTreeMap<String, Names> = BTreeMap::new();
    let mut pending: Vec<(String, Names)> = entries(resolver, files, dir)
        .into_iter()
        .map(|file| (file, Names::All))
        .collect();
    while let Some((file, names)) = pending.pop() {
        let Some(exposed) = grown(&mut reached, &file, names) else {
            continue;
        };
        for import in files.of(&file).imports {
            if import.kind != EdgeKind::ReExport {
                continue;
            }
            if let ImportTarget::File { path } =
                resolver.resolve(packages, &file, &import.specifier)
            {
                pending.push((path, passed(&import, &exposed)));
            }
        }
    }
    reached
}

/// What `file` exposes once `names` are added to what `reached` holds for it; `None` when
/// that adds nothing.
fn grown(reached: &mut BTreeMap<String, Names>, file: &str, names: Names) -> Option<Names> {
    match reached.get_mut(file) {
        Some(known) => known.absorb(names).then(|| known.clone()),
        None => Some(reached.entry(file.to_owned()).or_insert(names).clone()),
    }
}

/// What a re-export exposes of its target when its file exposes `exposed`.
fn passed(import: &RawImport, exposed: &Names) -> Names {
    if import.names.is_empty() {
        return exposed.clone();
    }
    Names::Only(
        import
            .names
            .iter()
            .filter(|name| exposed.allows(Some(outer(import, name))))
            .cloned()
            .collect(),
    )
}

/// The name a re-export exports `name` of its target as: its alias, else itself.
fn outer<'i>(import: &'i RawImport, name: &'i str) -> &'i str {
    import
        .aliases
        .iter()
        .find(|alias| alias.name == name)
        .map_or(name, |alias| alias.local.as_str())
}

/// The top-level name of `item` in its file: its path's first segment after the file's
/// namespace; `None` when its path does not start with the namespace.
fn top_name<'i>(item: &'i ApiItem, rules: &NamespaceRules) -> Option<&'i str> {
    let namespace = file_namespace(rules, &item.file).join(&rules.separator);
    let rest = if namespace.is_empty() {
        item.path.as_str()
    } else {
        item.path
            .strip_prefix(&namespace)?
            .strip_prefix(&rules.separator)?
    };
    rest.split(rules.separator.as_str()).next()
}

/// The entry files of the package in `dir`, as its manifest names them.
fn entries(resolver: &Resolver<'_, '_>, files: &Imports<'_>, dir: &str) -> Vec<String> {
    let Some(manifest) = resolver.json(&join(dir, "package.json")) else {
        return Vec::new();
    };
    let mut named = Vec::new();
    match manifest.get("exports") {
        Some(exports) => targets(exports, &mut named),
        None => {
            for key in ["main", "types", "typings"] {
                named.extend(manifest.get(key).and_then(Value::as_str).map(str::to_owned));
            }
        }
    }
    named
        .iter()
        .flat_map(|target| sources(resolver, files, dir, target))
        .collect()
}

/// Every target string of an `exports` value, through arrays, subpaths, and conditions.
fn targets(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(target) => out.push(target.clone()),
        Value::Array(choices) => choices.iter().for_each(|choice| targets(choice, out)),
        Value::Object(map) => map.values().for_each(|nested| targets(nested, out)),
        _ => {}
    }
}

/// The source files `target`, relative to the package in `dir`, stands for.
fn sources(
    resolver: &Resolver<'_, '_>,
    files: &Imports<'_>,
    dir: &str,
    target: &str,
) -> Vec<String> {
    let path = normalize(&join(dir, target));
    if target.contains('*') {
        return matching(resolver, files, dir, &path);
    }
    resolver
        .probe(&path)
        .or_else(|| {
            built(dir, &path)
                .iter()
                .find_map(|source| resolver.probe(source))
        })
        .into_iter()
        .collect()
}

/// The paths under `src` that the built output at `path` may stand for, without extension,
/// longest first; none when `path` is already under `src`.
fn built(dir: &str, path: &str) -> Vec<String> {
    let below = if dir == "." {
        path
    } else {
        path.strip_prefix(&format!("{dir}/")).unwrap_or(path)
    };
    let stem = BUILT
        .iter()
        .find_map(|extension| below.strip_suffix(extension))
        .unwrap_or(below);
    let segments: Vec<&str> = stem.split('/').collect();
    if segments.first() == Some(&SOURCES) {
        return Vec::new();
    }
    (1..segments.len())
        .map(|at| join(dir, &format!("{SOURCES}/{}", segments[at..].join("/"))))
        .collect()
}

/// The files of the language that the wildcard `pattern` matches, else, for built output,
/// those whose path without extension a pattern under `src` matches.
fn matching(
    resolver: &Resolver<'_, '_>,
    files: &Imports<'_>,
    dir: &str,
    pattern: &str,
) -> Vec<String> {
    let found = under(resolver, files, pattern, str::to_owned);
    if !found.is_empty() {
        return found;
    }
    built(dir, pattern)
        .iter()
        .map(|stem| under(resolver, files, stem, without_extension))
        .find(|found| !found.is_empty())
        .unwrap_or_default()
}

/// The files of the language below `pattern`'s fixed prefix whose `key` it matches.
fn under(
    resolver: &Resolver<'_, '_>,
    files: &Imports<'_>,
    pattern: &str,
    key: fn(&str) -> String,
) -> Vec<String> {
    let prefix = pattern
        .split_once('*')
        .map_or(pattern, |(prefix, _)| prefix);
    let top = prefix.rsplit_once('/').map_or(".", |(dir, _)| dir);
    let mut found = Vec::new();
    let mut dirs = vec![top.to_owned()];
    while let Some(dir) = dirs.pop() {
        dirs.extend(files.dirs_in(&dir).into_iter().filter(|sub| {
            let name = sub.rsplit('/').next().unwrap_or(sub);
            name != "node_modules" && !name.starts_with('.')
        }));
        found.extend(files.files_in(&dir, "").into_iter().filter(|file| {
            resolver.extensions().iter().any(|ext| file.ends_with(ext))
                && match_pattern(pattern, &key(file)).is_some()
        }));
    }
    found.sort();
    found
}

/// `file` without its extension.
fn without_extension(file: &str) -> String {
    let name_at = file.rfind('/').map_or(0, |at| at + 1);
    file[name_at..]
        .find('.')
        .map_or(file, |dot| &file[..name_at + dot])
        .to_owned()
}
