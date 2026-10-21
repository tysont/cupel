// ABOUTME: Clojure module resolution for `ModuleGraph`: source roots from `ns` forms and `deps.edn` paths,
// ABOUTME: each required namespace mapped to its file, Clojure's own namespaces and Java classes external.
//!
//! Queries find required namespaces; which file defines one depends on the classpath's source
//! roots, which a query cannot see, so this part of the language is hand-written (ADR-109).
//!
//! - **Source roots** are, for every file in scope, its path with the namespace its `ns` form
//!   declares (`ledger.core` is `ledger/core`) removed from its end; and, for every `deps.edn` of
//!   the repository, the directories its `:paths` and `:extra-paths` name (`src` when it names
//!   none), relative to it.
//! - **A namespace** `a.b-c` is the file `a/b_c.clj` or `a/b_c.cljc` in some source root, the
//!   first found in root order: dots separate directories and dashes become underscores.
//! - **Clojure's own namespaces** (`clojure.*`) are `External` with package `clojure`; **Java
//!   packages and classes** from `:import` in the JDK's packages (`java.`, `javax.`, `jdk.`) are
//!   `External` with package `jdk`.
//! - **Anything else** is `Unresolved` when a prefix of it names a directory of Clojure files in
//!   a source root (a namespace the repository should define: generated code, a typo), and
//!   otherwise `External` with the namespace itself as its package.
//! - Edges are grouped by project directory: the nearest directory above the file with a
//!   `deps.edn` (`.` outside every project). Clojure requires by namespace, never by path.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use cupel_core::ProducerRegistry;
use cupel_core::syntax::{ImportEdge, ImportTarget, ModuleGraph, RootImports};

use crate::imports::{Imports, join};
use crate::language::Language;
use crate::parse::ParseCache;

/// The extensions of Clojure files a namespace may live in, in the order the loader tries them.
const EXTENSIONS: [&str; 2] = [".clj", ".cljc"];

/// The manifest of a Clojure CLI project.
const MANIFEST: &str = "deps.edn";

/// Directories never searched for manifests: build output and tool state.
const GENERATED: [&str; 3] = ["target", ".cpcache", "node_modules"];

/// How deep below the repository root `deps.edn` files are searched for.
const SEARCH_DEPTH: usize = 6;

/// Registers the module graph and the references resolved through it, which read the file
/// system, since which file defines a namespace depends on the classpath's roots.
pub fn register(language: Arc<dyn Language>, cache: &Arc<ParseCache>, reg: &mut ProducerRegistry) {
    crate::references::register_resolved(language, cache, graph, reg);
}

/// Resolves every import in scope.
pub fn graph(imports: &Imports<'_>) -> ModuleGraph {
    let roots = source_roots(imports);
    let mut grouped: BTreeMap<String, Vec<ImportEdge>> = BTreeMap::new();
    for file in imports.scope() {
        let edges = grouped.entry(project_of(imports, file)).or_default();
        for import in imports.of(file).imports {
            edges.push(ImportEdge {
                from: file.clone(),
                target: resolve(imports, &roots, &import.specifier),
                specifier: import.specifier,
                relative: false,
                line: import.line,
                edge_kind: import.kind,
            });
        }
    }
    ModuleGraph {
        roots: grouped
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

/// What the required namespace (or imported Java name) `name` refers to.
fn resolve(imports: &Imports<'_>, roots: &[String], name: &str) -> ImportTarget {
    if let Some(path) = namespace_file(imports, roots, name) {
        return ImportTarget::File { path };
    }
    if name == "clojure" || name.starts_with("clojure.") {
        return ImportTarget::External {
            package: "clojure".to_owned(),
        };
    }
    if ["java.", "javax.", "jdk."]
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        return ImportTarget::External {
            package: "jdk".to_owned(),
        };
    }
    let segments: Vec<String> = name.split('.').map(file_segment).collect();
    let declared = (1..segments.len()).any(|length| {
        let dir = segments[..length].join("/");
        roots.iter().any(|root| {
            EXTENSIONS
                .iter()
                .any(|extension| !imports.files_in(&join(root, &dir), extension).is_empty())
        })
    });
    if declared {
        ImportTarget::Unresolved
    } else {
        ImportTarget::External {
            package: name.to_owned(),
        }
    }
}

/// The file defining the namespace `name` in the first source root that has one.
fn namespace_file(imports: &Imports<'_>, roots: &[String], name: &str) -> Option<String> {
    let path = name
        .split('.')
        .map(file_segment)
        .collect::<Vec<_>>()
        .join("/");
    roots.iter().find_map(|root| {
        EXTENSIONS
            .iter()
            .map(|extension| join(root, &format!("{path}{extension}")))
            .find(|file| imports.is_file(file))
    })
}

/// A namespace segment as the loader spells it in a path: dashes become underscores.
fn file_segment(segment: &str) -> String {
    segment.replace('-', "_")
}

/// Every source root, in path order: those the files in scope declare through their `ns` forms,
/// then those every `deps.edn` names.
fn source_roots(imports: &Imports<'_>) -> Vec<String> {
    let mut roots: BTreeSet<String> = imports
        .scope()
        .iter()
        .filter_map(|file| declared_root(file, &namespace_of(&imports.read(file)?)?))
        .collect();
    let mut manifests = Vec::new();
    manifests_below(imports, ".", 0, &mut manifests);
    for dir in manifests {
        let text = imports.read(&join(&dir, MANIFEST)).unwrap_or_default();
        let mut paths = paths_of(&text);
        if paths.is_empty() {
            paths.push("src".to_owned());
        }
        roots.extend(
            paths
                .iter()
                .map(|path| join(&dir, path))
                .filter(|path| imports.is_dir(path)),
        );
    }
    roots.into_iter().collect()
}

/// The directory `file` sits in with the namespace's path removed from its end; `None` when the
/// file is not where its namespace says.
fn declared_root(file: &str, namespace: &str) -> Option<String> {
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    let path = namespace
        .split('.')
        .map(file_segment)
        .collect::<Vec<_>>()
        .join("/");
    if stem == path {
        return Some(".".to_owned());
    }
    stem.strip_suffix(&format!("/{path}")).map(str::to_owned)
}

/// The namespace a file's `ns` form declares: the first symbol after `(ns`, metadata skipped;
/// `None` without an `ns` form.
fn namespace_of(text: &str) -> Option<String> {
    let code: String = text
        .lines()
        .map(|line| line.split(';').next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
    let start = code.find("(ns")? + 3;
    let rest = &code[start..];
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    rest.split_whitespace()
        .find(|word| !word.starts_with('^'))
        .map(|word| word.trim_end_matches(')').to_owned())
        .filter(|word| !word.is_empty())
}

/// The strings in every `:paths` and `:extra-paths` vector of a `deps.edn`'s text, in order.
fn paths_of(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for key in [":paths", ":extra-paths"] {
        let mut rest = text;
        while let Some(at) = rest.find(key) {
            rest = &rest[at + key.len()..];
            let Some(open) = rest.trim_start().strip_prefix('[') else {
                continue;
            };
            let vector = open.split(']').next().unwrap_or_default();
            paths.extend(vector.split('"').skip(1).step_by(2).map(str::to_owned));
        }
    }
    paths
}

/// Adds every directory at or below `dir` (to `SEARCH_DEPTH`) with a `deps.edn` to `found`,
/// skipping hidden and generated directories.
fn manifests_below(imports: &Imports<'_>, dir: &str, depth: usize, found: &mut Vec<String>) {
    if imports.is_file(&join(dir, MANIFEST)) {
        found.push(dir.to_owned());
    }
    if depth == SEARCH_DEPTH {
        return;
    }
    for child in imports.dirs_in(dir) {
        let name = child.rsplit('/').next().unwrap_or_default();
        if !name.starts_with('.') && !GENERATED.contains(&name) {
            manifests_below(imports, &child, depth + 1, found);
        }
    }
}

/// The project directory of `file`: the nearest directory above it with a `deps.edn`, `.` when
/// none has one.
fn project_of(imports: &Imports<'_>, file: &str) -> String {
    std::path::Path::new(file)
        .ancestors()
        .skip(1)
        .map(|dir| dir.to_string_lossy().into_owned())
        .find(|dir| imports.is_file(&join(dir, MANIFEST)))
        .filter(|dir| !dir.is_empty())
        .unwrap_or_else(|| ".".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{declared_root, namespace_of, paths_of};

    #[test]
    fn an_ns_form_names_its_namespace_past_comments_and_metadata() {
        let text =
            ";; Ledger.\n(ns ^:no-doc ledger.core-test\n  (:require [clojure.test :refer :all]))\n";
        assert_eq!(namespace_of(text).as_deref(), Some("ledger.core-test"));
        assert_eq!(namespace_of("(ns a.b)").as_deref(), Some("a.b"));
        assert_eq!(namespace_of("(defn f [])\n"), None);
        assert_eq!(namespace_of("(nsx a)"), None);
    }

    #[test]
    fn a_source_root_is_the_path_above_the_namespace() {
        assert_eq!(
            declared_root("src/ledger/core_test.clj", "ledger.core-test").as_deref(),
            Some("src")
        );
        assert_eq!(declared_root("core.clj", "core").as_deref(), Some("."));
        assert_eq!(declared_root("src/elsewhere.clj", "ledger.core"), None);
    }

    #[test]
    fn deps_edn_paths_and_extra_paths_are_read_in_order() {
        let cases: [(&str, &[&str]); 3] = [
            (
                "{:paths [\"src\" \"resources\"]\n :aliases {:test {:extra-paths [\"test\"]}}}\n",
                &["src", "resources", "test"],
            ),
            ("{:aliases {:dev {:extra-paths [\"dev\"]}}}", &["dev"]),
            ("{:deps {}}", &[]),
        ];
        for (deps, paths) in cases {
            assert_eq!(paths_of(deps), paths, "{deps}");
        }
    }
}
