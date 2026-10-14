// ABOUTME: Python module resolution for `ModuleGraph`: projects from pyproject/setup manifests and uv
// ABOUTME: workspaces, per-file import roots (src and flat layouts), packages, and relative imports.
//!
//! Queries find `import` and `from … import` statements; resolving them needs the directory
//! layout, which a query cannot see, so this part of the language is hand-written (ADR-035).
//!
//! - **Projects** are directories with `pyproject.toml`, `setup.py`, or `setup.cfg`: the nearest
//!   above each file in scope, and the `[tool.uv.workspace] members` of every project found.
//! - **Import roots** of a file, in order: its base directory (the first ancestor without
//!   `__init__.py`, which pytest puts on `sys.path` for a test and which is a script's own
//!   directory), its project's `src/` and directory (the repository root's for files outside
//!   every project), then every other project's `src/` and directory.
//! - **Absolute imports** are resolved in the first root that contains their first segment;
//!   one that no root contains is `External`, named by that segment. **Relative imports**
//!   start at the importing file's directory, one level up per extra dot.
//! - A module path resolves segment by segment: a regular package (`x/__init__.py`), else a
//!   module file (`x.py`, only as the last segment), else a namespace package (`x/`). The
//!   target is the imported name's own file when it is a submodule, else the module's file;
//!   a path that does not lead to a file is `Unresolved`. Names compare case-sensitively, as
//!   Python's importer does, even on case-insensitive file systems.
use std::collections::{BTreeMap, BTreeSet};

use cupel_core::syntax::{ImportEdge, ImportTarget, ModuleGraph, RootImports};

use crate::imports::{Imports, RawImport, join};

/// Files whose presence makes a directory a project.
const MANIFESTS: [&str; 3] = ["pyproject.toml", "setup.py", "setup.cfg"];

/// Resolves every import in scope.
pub fn graph(imports: &Imports<'_>) -> ModuleGraph {
    let projects = projects(imports);
    let mut roots: BTreeMap<String, Vec<ImportEdge>> = projects
        .iter()
        .map(|project| (project.clone(), Vec::new()))
        .collect();
    for file in imports.scope() {
        let project = project_of(&projects, file).unwrap_or(".");
        let search = import_roots(imports, &projects, project, file);
        let edges = roots.entry(project.to_owned()).or_default();
        for import in &imports.of(file).imports {
            edges.push(edge(imports, &search, file, import));
        }
    }
    ModuleGraph {
        roots: roots
            .into_iter()
            .map(|(root, mut edges)| {
                edges.sort_by(|a, b| {
                    (&a.from, a.line, &a.specifier).cmp(&(&b.from, b.line, &b.specifier))
                });
                RootImports { root, edges }
            })
            .collect(),
    }
}

/// The edge one imported module or name contributes.
fn edge(imports: &Imports<'_>, roots: &[String], file: &str, import: &RawImport) -> ImportEdge {
    let dots = import.specifier.len() - import.specifier.trim_start_matches('.').len();
    let segments: Vec<&str> = import.specifier[dots..]
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect();
    let name = import.names.first().map(String::as_str);
    let specifier = match name {
        Some(name) if import.specifier.ends_with('.') => format!("{}{name}", import.specifier),
        Some(name) => format!("{}.{name}", import.specifier),
        None => import.specifier.clone(),
    };
    let target = if dots > 0 {
        (1..dots)
            .try_fold(dir_of(file), |dir, _| parent(&dir))
            .and_then(|start| resolve_below(imports, &start, &segments, name))
            .map_or(ImportTarget::Unresolved, |path| ImportTarget::File { path })
    } else {
        absolute(imports, roots, &segments, name)
    };
    ImportEdge {
        from: file.to_owned(),
        specifier,
        target,
        relative: dots > 0,
        line: import.line,
    }
}

/// Where an absolute module path leads: resolved in the first root that has its first segment.
fn absolute(
    imports: &Imports<'_>,
    roots: &[String],
    segments: &[&str],
    name: Option<&str>,
) -> ImportTarget {
    let Some(first) = segments.first() else {
        return ImportTarget::Unresolved;
    };
    let root = roots.iter().find(|root| {
        imports.is_file(&join(root, &format!("{first}.py"))) || imports.is_dir(&join(root, first))
    });
    match root {
        Some(root) => resolve_below(imports, root, segments, name)
            .map_or(ImportTarget::Unresolved, |path| ImportTarget::File { path }),
        None => ImportTarget::External {
            package: (*first).to_owned(),
        },
    }
}

/// The file `from <segments> import <name>` (or `import <segments>`) names below `start`.
fn resolve_below(
    imports: &Imports<'_>,
    start: &str,
    segments: &[&str],
    name: Option<&str>,
) -> Option<String> {
    let mut dir = start.to_owned();
    let mut module = package_file(imports, &dir);
    for (index, segment) in segments.iter().enumerate() {
        let file = format!("{segment}.py");
        if !imports.is_dir(&join(&dir, segment)) {
            return (imports.is_file(&join(&dir, &file)) && index + 1 == segments.len())
                .then(|| join(&dir, &file));
        }
        let child = join(&dir, segment);
        match package_file(imports, &child) {
            Some(init) => module = Some(init),
            // A module file beside a namespace directory of the same name wins over it.
            None if imports.is_file(&join(&dir, &file)) && index + 1 == segments.len() => {
                return Some(join(&dir, &file));
            }
            None => module = None,
        }
        dir = child;
    }
    name.filter(|name| *name != "*")
        .and_then(|name| {
            let file = format!("{name}.py");
            if imports.is_file(&join(&dir, &file)) {
                Some(join(&dir, &file))
            } else if imports.is_dir(&join(&dir, name)) {
                package_file(imports, &join(&dir, name))
            } else {
                None
            }
        })
        .or(module)
}

/// `dir/__init__.py`, when the directory `dir` is a regular package.
fn package_file(imports: &Imports<'_>, dir: &str) -> Option<String> {
    let init = join(dir, "__init__.py");
    imports.is_file(&init).then_some(init)
}

/// The directories on `sys.path` for `file`, in search order, without repeats.
fn import_roots(
    imports: &Imports<'_>,
    projects: &[String],
    project: &str,
    file: &str,
) -> Vec<String> {
    let mut base = dir_of(file);
    while package_file(imports, &base).is_some() {
        match parent(&base) {
            Some(up) => base = up,
            None => break,
        }
    }
    let mut roots = vec![base];
    let others = projects
        .iter()
        .map(String::as_str)
        .filter(|p| *p != project);
    for dir in std::iter::once(project).chain(others) {
        if imports.is_dir(&join(dir, "src")) {
            roots.push(join(dir, "src"));
        }
        roots.push(dir.to_owned());
    }
    let mut seen = BTreeSet::new();
    roots.retain(|root| seen.insert(root.clone()));
    roots
}

/// The nearest project above each file in scope, and their uv workspace members.
fn projects(imports: &Imports<'_>) -> Vec<String> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    for file in imports.scope() {
        let mut dir = Some(dir_of(file));
        while let Some(current) = dir {
            if is_project(imports, &current) {
                found.insert(current);
                break;
            }
            dir = parent(&current);
        }
    }
    let mut queue: Vec<String> = found.iter().cloned().collect();
    while let Some(project) = queue.pop() {
        for member in workspace_members(imports, &project) {
            if is_project(imports, &member) && found.insert(member.clone()) {
                queue.push(member);
            }
        }
    }
    found.into_iter().collect()
}

fn is_project(imports: &Imports<'_>, dir: &str) -> bool {
    MANIFESTS
        .iter()
        .any(|manifest| imports.is_file(&join(dir, manifest)))
}

/// The project containing `file`: the one with the longest directory prefix.
fn project_of<'a>(projects: &'a [String], file: &str) -> Option<&'a str> {
    projects
        .iter()
        .filter(|p| *p == "." || file.starts_with(&format!("{p}/")))
        .max_by_key(|p| if *p == "." { 0 } else { p.len() })
        .map(String::as_str)
}

/// The directories `[tool.uv.workspace] members` names in `project`'s `pyproject.toml`.
fn workspace_members(imports: &Imports<'_>, project: &str) -> Vec<String> {
    let Some(manifest) = imports
        .read(&join(project, "pyproject.toml"))
        .and_then(|text| text.parse::<toml::Table>().ok())
    else {
        return Vec::new();
    };
    let members = manifest
        .get("tool")
        .and_then(|tool| tool.get("uv"))
        .and_then(|uv| uv.get("workspace"))
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    members
        .iter()
        .filter_map(toml::Value::as_str)
        .flat_map(|pattern| imports.expand(project, pattern))
        .collect()
}

/// The directory of `file`, `.` at the root.
fn dir_of(file: &str) -> String {
    file.rsplit_once('/')
        .map_or_else(|| ".".to_owned(), |(dir, _)| dir.to_owned())
}

/// The parent of `dir`, `None` above the root.
fn parent(dir: &str) -> Option<String> {
    match dir.rsplit_once('/') {
        _ if dir == "." => None,
        Some((up, _)) => Some(up.to_owned()),
        None => Some(".".to_owned()),
    }
}
