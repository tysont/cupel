// ABOUTME: Go module resolution for `ModuleGraph`: modules from go.mod and go.work, import paths mapped
// ABOUTME: to the files of the package directory they name, the standard library and required modules external.
//!
//! Queries find import paths; what a path names depends on `go.mod` and `go.work`, which a
//! query cannot see, so this part of the language is hand-written.
//!
//! - **Modules** are the directories whose `go.mod` has a `module` line: the repository root's,
//!   the `use` directories of the root `go.work`, and the nearest one above each file in scope.
//! - **An import path inside a module** (the module path itself or below it, the longest
//!   module path winning) names a package directory in that module. A package is every
//!   non-test `.go` file directly in it, so the import has one edge to each of them, in path
//!   order; a directory without such files is `Unresolved`. Any module found in the repository
//!   resolves this way, whether or not the importing module requires it.
//! - **The standard library** is every path whose first element has no dot; it is `External`
//!   with package `std`.
//! - **A required module**: a path inside a module the importing `go.mod` requires (the longest
//!   wins) is `External` with that module path as its package. Anything else is `Unresolved`.
//! - Edges are grouped by module directory (`.` for files outside every module). Go modules
//!   have no relative imports, so no edge is `relative`.
use std::collections::{BTreeMap, BTreeSet};

use cupel_core::syntax::{ImportEdge, ImportTarget, ModuleGraph, RootImports};

use crate::imports::{Imports, join};

/// The package Go's standard library is reported as.
const STANDARD: &str = "std";

/// A Go module.
#[derive(Debug, Clone)]
struct Module {
    dir: String,
    path: String,
    requires: Vec<String>,
}

/// Resolves every import in scope.
pub fn graph(imports: &Imports<'_>) -> ModuleGraph {
    let modules = modules(imports);
    let mut roots: BTreeMap<String, Vec<ImportEdge>> = modules
        .iter()
        .map(|module| (module.dir.clone(), Vec::new()))
        .collect();
    for file in imports.scope() {
        let module = module_of(&modules, file);
        let group = module.map_or_else(|| ".".to_owned(), |m| m.dir.clone());
        let edges = roots.entry(group).or_default();
        for import in imports.of(file).imports {
            for target in resolve(imports, &modules, module, &import.specifier) {
                edges.push(ImportEdge {
                    from: file.clone(),
                    specifier: import.specifier.clone(),
                    target,
                    relative: false,
                    line: import.line,
                });
            }
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

/// What the import path `path`, written in `importer`, names.
fn resolve(
    imports: &Imports<'_>,
    modules: &[Module],
    importer: Option<&Module>,
    path: &str,
) -> Vec<ImportTarget> {
    let local = modules
        .iter()
        .filter_map(|module| within(path, &module.path).map(|rest| (module, rest)))
        .max_by_key(|(module, _)| module.path.len());
    if let Some((module, rest)) = local {
        let dir = if rest.is_empty() {
            module.dir.clone()
        } else {
            join(&module.dir, rest)
        };
        let files: Vec<ImportTarget> = imports
            .files_in(&dir, ".go")
            .into_iter()
            .filter(|file| !file.ends_with("_test.go"))
            .map(|path| ImportTarget::File { path })
            .collect();
        return if files.is_empty() {
            vec![ImportTarget::Unresolved]
        } else {
            files
        };
    }
    if !path.split('/').next().unwrap_or_default().contains('.') {
        return vec![ImportTarget::External {
            package: STANDARD.to_owned(),
        }];
    }
    let required = importer.and_then(|module| {
        module
            .requires
            .iter()
            .filter(|required| within(path, required).is_some())
            .max_by_key(|required| required.len())
    });
    vec![
        required.map_or(ImportTarget::Unresolved, |package| ImportTarget::External {
            package: package.clone(),
        }),
    ]
}

/// The part of `path` below the module path `module` (empty for the module itself).
fn within<'a>(path: &'a str, module: &str) -> Option<&'a str> {
    if path == module {
        Some("")
    } else {
        path.strip_prefix(module)?.strip_prefix('/')
    }
}

/// The module that contains `file`: the one with the longest directory prefix.
fn module_of<'a>(modules: &'a [Module], file: &str) -> Option<&'a Module> {
    modules
        .iter()
        .filter(|m| m.dir == "." || file.starts_with(&format!("{}/", m.dir)))
        .max_by_key(|m| if m.dir == "." { 0 } else { m.dir.len() })
}

/// The root module, the root workspace's members, and the nearest module above each file.
fn modules(imports: &Imports<'_>) -> Vec<Module> {
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    dirs.insert(".".to_owned());
    if let Some(work) = imports.read("go.work") {
        dirs.extend(directives(&work, "use").iter().map(|dir| normalize(dir)));
    }
    for file in imports.scope() {
        let mut dir = file.rsplit_once('/').map_or("", |(dir, _)| dir).to_owned();
        loop {
            let at = if dir.is_empty() { "." } else { dir.as_str() };
            if imports.is_file(&join(at, "go.mod")) {
                dirs.insert(at.to_owned());
                break;
            }
            match dir.rsplit_once('/') {
                Some((parent, _)) => dir = parent.to_owned(),
                None if !dir.is_empty() => dir.clear(),
                None => break,
            }
        }
    }
    dirs.into_iter()
        .filter_map(|dir| {
            let manifest = imports.read(&join(&dir, "go.mod"))?;
            let path = directives(&manifest, "module").into_iter().next()?;
            Some(Module {
                requires: directives(&manifest, "require"),
                dir,
                path,
            })
        })
        .collect()
}

/// The first argument of every `keyword` directive in a go.mod or go.work file, in single-line
/// (`require a v1`) and block (`require ( … )`) form, unquoted, comments removed.
fn directives(text: &str, keyword: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut in_block = false;
    for line in text.lines() {
        let line = line.split("//").next().unwrap_or_default().trim();
        let mut words = line.split_whitespace();
        if in_block {
            match words.next() {
                Some(")") => in_block = false,
                Some(first) => found.push(unquote(first)),
                None => {}
            }
            continue;
        }
        if words.next() != Some(keyword) {
            continue;
        }
        match words.next() {
            Some("(") => in_block = true,
            Some(first) => found.push(unquote(first)),
            None => {}
        }
    }
    found
}

fn unquote(text: &str) -> String {
    text.trim_matches(|c| c == '"' || c == '`').to_owned()
}

/// `./a/../b` as `b`, and `./` as `.`.
fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

#[cfg(test)]
mod tests {
    use super::{directives, normalize};

    #[test]
    fn directives_read_single_lines_and_blocks_without_comments() {
        let manifest = "module \"example.com/m\" // the module\n\nrequire a.io/x v1.0.0\nrequire (\n\tb.io/y v2.0.0 // indirect\n\n)\n";
        assert_eq!(directives(manifest, "module"), ["example.com/m"]);
        assert_eq!(directives(manifest, "require"), ["a.io/x", "b.io/y"]);
        assert!(directives(manifest, "replace").is_empty());
    }

    #[test]
    fn workspace_paths_normalize() {
        assert_eq!(normalize("./app"), "app");
        assert_eq!(normalize("."), ".");
        assert_eq!(normalize("./a/../b/"), "b");
    }
}
