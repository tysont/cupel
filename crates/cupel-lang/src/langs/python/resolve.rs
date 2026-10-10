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
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use cupel_core::syntax::{ImportEdge, ImportTarget, ModuleGraph, RootImports};
use globset::Glob;

use crate::imports::{Imports, RawImport, join};

/// Files whose presence makes a directory a project.
const MANIFESTS: [&str; 3] = ["pyproject.toml", "setup.py", "setup.cfg"];

/// Resolves every import in scope.
pub fn graph(imports: &Imports<'_>) -> ModuleGraph {
    let projects = projects(imports);
    let layout = Layout::new(imports);
    let mut roots: BTreeMap<String, Vec<ImportEdge>> = projects
        .iter()
        .map(|project| (project.clone(), Vec::new()))
        .collect();
    for file in imports.scope() {
        let project = project_of(&projects, file).unwrap_or(".");
        let search = import_roots(&layout, &projects, project, file);
        let edges = roots.entry(project.to_owned()).or_default();
        for import in &imports.of(file).imports {
            edges.push(edge(&layout, &search, file, import));
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
fn edge(layout: &Layout<'_>, roots: &[String], file: &str, import: &RawImport) -> ImportEdge {
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
            .and_then(|start| resolve_below(layout, &start, &segments, name))
            .map_or(ImportTarget::Unresolved, |path| ImportTarget::File { path })
    } else {
        absolute(layout, roots, &segments, name)
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
    layout: &Layout<'_>,
    roots: &[String],
    segments: &[&str],
    name: Option<&str>,
) -> ImportTarget {
    let Some(first) = segments.first() else {
        return ImportTarget::Unresolved;
    };
    let root = roots
        .iter()
        .find(|root| layout.is_file(root, &format!("{first}.py")) || layout.is_dir(root, first));
    match root {
        Some(root) => resolve_below(layout, root, segments, name)
            .map_or(ImportTarget::Unresolved, |path| ImportTarget::File { path }),
        None => ImportTarget::External {
            package: (*first).to_owned(),
        },
    }
}

/// The file `from <segments> import <name>` (or `import <segments>`) names below `start`.
fn resolve_below(
    layout: &Layout<'_>,
    start: &str,
    segments: &[&str],
    name: Option<&str>,
) -> Option<String> {
    let mut dir = start.to_owned();
    let mut module = layout.package_file(&dir);
    for (index, segment) in segments.iter().enumerate() {
        let file = format!("{segment}.py");
        if !layout.is_dir(&dir, segment) {
            return (layout.is_file(&dir, &file) && index + 1 == segments.len())
                .then(|| join(&dir, &file));
        }
        let child = join(&dir, segment);
        match layout.package_file(&child) {
            Some(init) => module = Some(init),
            // A module file beside a namespace directory of the same name wins over it.
            None if layout.is_file(&dir, &file) && index + 1 == segments.len() => {
                return Some(join(&dir, &file));
            }
            None => module = None,
        }
        dir = child;
    }
    name.filter(|name| *name != "*")
        .and_then(|name| {
            let file = format!("{name}.py");
            if layout.is_file(&dir, &file) {
                Some(join(&dir, &file))
            } else if layout.is_dir(&dir, name) {
                layout.package_file(&join(&dir, name))
            } else {
                None
            }
        })
        .or(module)
}

/// One directory's entries: files and subdirectories, as paths from the repository root.
type Listing = (BTreeSet<String>, BTreeSet<String>);

/// The repository's directories as listed, cached per directory; names compare exactly.
struct Layout<'a> {
    imports: &'a Imports<'a>,
    listings: RefCell<HashMap<String, Listing>>,
}

impl<'a> Layout<'a> {
    fn new(imports: &'a Imports<'a>) -> Self {
        Self {
            imports,
            listings: RefCell::new(HashMap::new()),
        }
    }

    /// Whether `dir` holds a file called exactly `name`.
    fn is_file(&self, dir: &str, name: &str) -> bool {
        self.listed(dir, |(files, _)| files.contains(&join(dir, name)))
    }

    /// Whether `dir` holds a directory called exactly `name`.
    fn is_dir(&self, dir: &str, name: &str) -> bool {
        self.listed(dir, |(_, dirs)| dirs.contains(&join(dir, name)))
    }

    /// `dir/__init__.py`, when the directory `dir` is a regular package.
    fn package_file(&self, dir: &str) -> Option<String> {
        self.is_file(dir, "__init__.py")
            .then(|| join(dir, "__init__.py"))
    }

    fn listed(&self, dir: &str, check: impl Fn(&Listing) -> bool) -> bool {
        let mut listings = self.listings.borrow_mut();
        let listing = listings.entry(dir.to_owned()).or_insert_with(|| {
            (
                self.imports.files_in(dir, "").into_iter().collect(),
                self.imports.dirs_in(dir).into_iter().collect(),
            )
        });
        check(listing)
    }
}

/// The directories on `sys.path` for `file`, in search order, without repeats.
fn import_roots(
    layout: &Layout<'_>,
    projects: &[String],
    project: &str,
    file: &str,
) -> Vec<String> {
    let mut base = dir_of(file);
    while layout.package_file(&base).is_some() {
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
        if layout.is_dir(dir, "src") {
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
        .flat_map(|pattern| expand(imports, project, pattern))
        .collect()
}

/// The directories below `base` a member pattern names; `*` and `?` match one path segment.
fn expand(imports: &Imports<'_>, base: &str, pattern: &str) -> Vec<String> {
    let mut dirs = vec![base.to_owned()];
    for segment in pattern.split('/').filter(|s| !s.is_empty() && *s != ".") {
        let matcher = segment
            .contains(['*', '?', '['])
            .then(|| Glob::new(segment).ok().map(|glob| glob.compile_matcher()))
            .flatten();
        dirs = dirs
            .iter()
            .flat_map(|dir| {
                if let Some(matcher) = &matcher {
                    imports
                        .dirs_in(dir)
                        .into_iter()
                        .filter(|child| {
                            let name = child.rsplit_once('/').map_or(child.as_str(), |(_, n)| n);
                            matcher.is_match(name)
                        })
                        .collect()
                } else {
                    let child = join(dir, segment);
                    if imports.is_dir(&child) {
                        vec![child]
                    } else {
                        Vec::new()
                    }
                }
            })
            .collect();
    }
    dirs
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
