// ABOUTME: Rust module resolution for `ModuleGraph`: packages from Cargo.toml (workspace members, path
// ABOUTME: and renamed dependencies), the `mod` tree from each crate root, and `use` paths resolved in it.
//!
//! Queries find `mod` declarations and `use` trees; resolving them needs the crate layout,
//! which a query cannot see, so this part of the language is hand-written.
//!
//! - **Packages** are the directories whose `Cargo.toml` has a `[package]`: the root manifest's,
//!   its `[workspace] members` (globs as `Imports::expand` reads them), and the nearest one above each file in scope.
//! - **Crate roots** of a package: `src/lib.rs` first, then `src/main.rs`, `src/bin/*.rs`,
//!   `src/bin/*/main.rs`, `tests/*.rs`, `benches/*.rs`, `examples/*.rs`, and `build.rs`. A file
//!   no root reaches through `mod` declarations is treated as a crate root of its own.
//! - **`mod a;`** resolves to `a.rs` or `a/mod.rs` next to the declaring file (inside `x/` for
//!   a non-root, non-`mod.rs` file `x.rs`, and inside the inline modules around it). `#[path]`
//!   attributes are not read.
//! - **`use` paths** start at `crate`, `self`, `super`, a child module (2018 uniform paths), a
//!   dependency (a path dependency resolves into its crate, any other is `External`), the
//!   package's own library (from tests and binaries), a workspace package, or the standard
//!   library (`External`); anything else is `Unresolved`. The target is the file of the deepest
//!   module the path names; items inside a module resolve to the module's file.
//! - Edges are grouped by package directory (`.` for files outside every package). Paths to
//!   modules (`self::`, `super::`, child modules, `mod`) are `relative`.
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use cupel_core::syntax::{EdgeKind, ImportEdge, ImportTarget, ModuleGraph, RootImports};

use crate::imports::{FileImports, Imports, RawImport, join};

/// Crates every Rust program can name without a dependency.
const STANDARD: [&str; 5] = ["std", "core", "alloc", "proc_macro", "test"];

/// A Cargo package.
#[derive(Debug, Clone)]
struct Package {
    dir: String,
    /// The package name as code spells it (`-` becomes `_`).
    name: String,
    deps: BTreeMap<String, Dep>,
}

#[derive(Debug, Clone)]
enum Dep {
    /// A path dependency: the package directory.
    Path(String),
    /// Any other dependency: the package name.
    Registry(String),
}

/// Where a file sits in a crate's module tree.
#[derive(Debug, Clone)]
struct Location {
    root: String,
    path: Vec<String>,
    children_dir: String,
}

/// Every crate's module tree.
#[derive(Default)]
struct Tree {
    files: HashMap<String, Location>,
    modules: HashMap<(String, Vec<String>), String>,
    imports: HashMap<String, FileImports>,
}

/// Resolves every `mod` and `use` in scope.
pub fn graph(imports: &Imports<'_>) -> ModuleGraph {
    let packages = packages(imports);
    let mut tree = Tree::default();
    for package in &packages {
        for root in crate_roots(imports, &package.dir) {
            tree.walk(imports, &root);
        }
    }
    for file in imports.scope() {
        if !tree.files.contains_key(file) {
            tree.walk(imports, file);
        }
    }

    let mut roots: BTreeMap<String, Vec<ImportEdge>> = packages
        .iter()
        .map(|package| (package.dir.clone(), Vec::new()))
        .collect();
    for file in imports.scope() {
        let package = package_of(&packages, file);
        let group = package.map_or_else(|| ".".to_owned(), |p| p.dir.clone());
        let edges = roots.entry(group).or_default();
        let Some(location) = tree.files.get(file).cloned() else {
            continue;
        };
        let file_imports = tree.imports_of(imports, file);
        let importer = Importer {
            imports,
            packages: &packages,
            package,
            file,
            location: &location,
        };
        for import in &file_imports.imports {
            edges.extend(tree.edges(&importer, import));
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

impl Tree {
    fn imports_of(&mut self, imports: &Imports<'_>, file: &str) -> FileImports {
        self.imports
            .entry(file.to_owned())
            .or_insert_with(|| imports.of(file))
            .clone()
    }

    /// Adds the crate rooted at `root` (unless a crate already reached it) and every module
    /// its `mod` declarations reach.
    fn walk(&mut self, imports: &Imports<'_>, root: &str) {
        if self.files.contains_key(root) {
            return;
        }
        self.add(root, root, Vec::new(), children_dir(root, true));
        let mut queue = VecDeque::from([root.to_owned()]);
        while let Some(file) = queue.pop_front() {
            let location = self.files[&file].clone();
            let declared = self.imports_of(imports, &file);
            for inline in &declared.modules {
                let path = [location.path.as_slice(), inline].concat();
                self.modules
                    .entry((location.root.clone(), path))
                    .or_insert_with(|| file.clone());
            }
            for import in declared
                .imports
                .iter()
                .filter(|import| import.kind == EdgeKind::Declaration)
            {
                let Some(child) = child_file(imports, &location.children_dir, import) else {
                    continue;
                };
                if self.files.contains_key(&child) {
                    continue;
                }
                let path = [
                    location.path.as_slice(),
                    &import.scope,
                    std::slice::from_ref(&import.specifier),
                ]
                .concat();
                let dir = children_dir(&child, false);
                self.add(&child, &location.root, path, dir);
                queue.push_back(child);
            }
        }
    }

    fn add(&mut self, file: &str, root: &str, path: Vec<String>, children_dir: String) {
        self.modules
            .insert((root.to_owned(), path.clone()), file.to_owned());
        self.files.insert(
            file.to_owned(),
            Location {
                root: root.to_owned(),
                path,
                children_dir,
            },
        );
    }

    /// The edges one import statement of `from` contributes.
    fn edges(&self, from: &Importer<'_>, import: &RawImport) -> Vec<ImportEdge> {
        let location = from.location;
        let edge = |specifier: String, target: ImportTarget, relative: bool| ImportEdge {
            from: from.file.to_owned(),
            specifier,
            target,
            relative,
            line: import.line,
            edge_kind: import.kind,
        };
        if import.kind == EdgeKind::Declaration {
            let target = child_file(from.imports, &location.children_dir, import)
                .map_or(ImportTarget::Unresolved, |path| ImportTarget::File { path });
            return vec![edge(import.specifier.clone(), target, true)];
        }
        let here = [location.path.as_slice(), &import.scope].concat();
        expand(&import.specifier)
            .into_iter()
            .map(|path| {
                let (target, relative) = self.resolve(from, &here, &path);
                edge(path.join("::"), target, relative)
            })
            .collect()
    }

    /// Where a `use` path written by `from` in module `here` of its crate leads, and whether
    /// it is relative to `here`.
    fn resolve(
        &self,
        from: &Importer<'_>,
        here: &[String],
        path: &[String],
    ) -> (ImportTarget, bool) {
        let root = from.location.root.as_str();
        match self.start(root, here, path) {
            Start::Within {
                base,
                skip,
                relative,
            } => (self.walk_down(root, &base, &path[skip..]), relative),
            Start::AboveRoot => (ImportTarget::Unresolved, true),
            Start::Outside(first) => {
                let target = match from.extern_crate(first) {
                    Extern::Crate(lib) => self.walk_down(&lib, &[], &path[1..]),
                    Extern::External(name) => ImportTarget::External { package: name },
                    Extern::Missing => ImportTarget::Unresolved,
                };
                (target, false)
            }
        }
    }

    /// Where `path`, written in module `here` of the crate rooted at `root`, starts.
    fn start<'p>(&self, root: &str, here: &[String], path: &'p [String]) -> Start<'p> {
        let Some(first) = path.first().map(String::as_str) else {
            return Start::Outside("");
        };
        let within = |base: Vec<String>, skip, relative| Start::Within {
            base,
            skip,
            relative,
        };
        match first {
            "crate" => within(Vec::new(), 1, false),
            "self" => within(here.to_vec(), 1, true),
            "super" => {
                let supers = path.iter().take_while(|s| *s == "super").count();
                if supers <= here.len() {
                    within(here[..here.len() - supers].to_vec(), supers, true)
                } else {
                    Start::AboveRoot
                }
            }
            _ if self.has(root, &[here, &[first.to_owned()]].concat()) => {
                within(here.to_vec(), 0, true)
            }
            _ => Start::Outside(first),
        }
    }

    /// The file of the deepest module `rest` names below `base` in the crate at `root`.
    fn walk_down(&self, root: &str, base: &[String], rest: &[String]) -> ImportTarget {
        let mut current = base.to_vec();
        for segment in rest {
            current.push(segment.clone());
            if !self.has(root, &current) {
                current.pop();
                break;
            }
        }
        self.modules
            .get(&(root.to_owned(), current))
            .map_or(ImportTarget::Unresolved, |path| ImportTarget::File {
                path: path.clone(),
            })
    }

    fn has(&self, root: &str, path: &[String]) -> bool {
        self.modules.contains_key(&(root.to_owned(), path.to_vec()))
    }
}

/// One file whose imports are being resolved, and the workspace around it.
struct Importer<'a> {
    imports: &'a Imports<'a>,
    packages: &'a [Package],
    /// The package the file belongs to.
    package: Option<&'a Package>,
    file: &'a str,
    location: &'a Location,
}

impl Importer<'_> {
    /// What a path's first segment names when it is not a module of this crate.
    fn extern_crate(&self, name: &str) -> Extern {
        let library = |dir: &str| {
            let lib = join(dir, "src/lib.rs");
            if self.imports.is_file(&lib) {
                Extern::Crate(lib)
            } else {
                Extern::Missing
            }
        };
        if let Some(dep) = self.package.and_then(|p| p.deps.get(name)) {
            return match dep {
                Dep::Path(dir) => library(dir),
                Dep::Registry(package) => Extern::External(package.clone()),
            };
        }
        if let Some(other) = self.packages.iter().find(|p| p.name == name) {
            return library(&other.dir);
        }
        if STANDARD.contains(&name) {
            return Extern::External(name.to_owned());
        }
        Extern::Missing
    }
}

/// Where a `use` path starts.
enum Start<'p> {
    /// In this crate: below `base`, after `skip` leading segments (`crate`, `self`, `super`).
    Within {
        base: Vec<String>,
        skip: usize,
        relative: bool,
    },
    /// More `super`s than the module is deep.
    AboveRoot,
    /// Another crate, named by the first segment.
    Outside(&'p str),
}

enum Extern {
    Crate(String),
    External(String),
    Missing,
}

/// The directory a file's `mod` declarations look in.
fn children_dir(file: &str, is_root: bool) -> String {
    let (dir, name) = file.rsplit_once('/').unwrap_or(("", file));
    if is_root || name == "mod.rs" {
        dir.to_owned()
    } else {
        join(dir, name.trim_end_matches(".rs"))
    }
}

/// The file a `mod` declaration names, if it exists.
fn child_file(imports: &Imports<'_>, children_dir: &str, import: &RawImport) -> Option<String> {
    let base = import
        .scope
        .iter()
        .fold(children_dir.to_owned(), |dir, inline| join(&dir, inline));
    let name = &import.specifier;
    [
        join(&base, &format!("{name}.rs")),
        join(&join(&base, name), "mod.rs"),
    ]
    .into_iter()
    .find(|candidate| imports.is_file(candidate))
}

/// The package that contains `file`: the one with the longest directory prefix.
fn package_of<'a>(packages: &'a [Package], file: &str) -> Option<&'a Package> {
    packages
        .iter()
        .filter(|p| p.dir == "." || file.starts_with(&format!("{}/", p.dir)))
        .max_by_key(|p| if p.dir == "." { 0 } else { p.dir.len() })
}

/// Every crate root of the package in `dir`, library first.
fn crate_roots(imports: &Imports<'_>, dir: &str) -> Vec<String> {
    let mut roots = Vec::new();
    for single in ["src/lib.rs", "src/main.rs"] {
        let path = join(dir, single);
        if imports.is_file(&path) {
            roots.push(path);
        }
    }
    roots.extend(imports.files_in(&join(dir, "src/bin"), ".rs"));
    for bin in imports.dirs_in(&join(dir, "src/bin")) {
        let main = join(&bin, "main.rs");
        if imports.is_file(&main) {
            roots.push(main);
        }
    }
    for kind in ["tests", "benches", "examples"] {
        roots.extend(imports.files_in(&join(dir, kind), ".rs"));
    }
    let build = join(dir, "build.rs");
    if imports.is_file(&build) {
        roots.push(build);
    }
    roots
}

/// The root package, workspace members, and the nearest package above each file in scope.
fn packages(imports: &Imports<'_>) -> Vec<Package> {
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    if let Some(manifest) = manifest(imports, ".") {
        if manifest.contains_key("package") {
            dirs.insert(".".to_owned());
        }
        let members = manifest
            .get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(toml::Value::as_array)
            .cloned()
            .unwrap_or_default();
        for member in members.iter().filter_map(toml::Value::as_str) {
            for dir in imports.expand(".", member) {
                dirs.insert(dir);
            }
        }
    }
    dirs.extend(
        imports
            .scope()
            .iter()
            .filter_map(|file| package_dir_of(imports, file)),
    );
    dirs.into_iter()
        .filter_map(|dir| package(imports, &dir))
        .collect()
}

/// The directory (`.` for the root) of the nearest `Cargo.toml` with a `[package]` at or above
/// `file`'s directory.
fn package_dir_of(imports: &Imports<'_>, file: &str) -> Option<String> {
    let mut dir = file.rsplit_once('/').map_or("", |(dir, _)| dir).to_owned();
    loop {
        let at = if dir.is_empty() { "." } else { dir.as_str() };
        if manifest(imports, at).is_some_and(|m| m.contains_key("package")) {
            return Some(at.to_owned());
        }
        match dir.rsplit_once('/') {
            Some((parent, _)) => dir = parent.to_owned(),
            None if !dir.is_empty() => dir.clear(),
            None => return None,
        }
    }
}

fn manifest(imports: &Imports<'_>, dir: &str) -> Option<toml::Table> {
    imports.read(&join(dir, "Cargo.toml"))?.parse().ok()
}

fn package(imports: &Imports<'_>, dir: &str) -> Option<Package> {
    let manifest = manifest(imports, dir)?;
    let name = manifest
        .get("package")?
        .get("name")?
        .as_str()?
        .replace('-', "_");
    let mut deps = BTreeMap::new();
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(table) = manifest.get(section).and_then(toml::Value::as_table) else {
            continue;
        };
        for (key, value) in table {
            let path = value.get("path").and_then(toml::Value::as_str);
            let package = value.get("package").and_then(toml::Value::as_str);
            let dep = match path {
                Some(path) => Dep::Path(normalize(&join(dir, path))),
                None => Dep::Registry(package.unwrap_or(key).to_owned()),
            };
            deps.insert(key.replace('-', "_"), dep);
        }
    }
    Some(Package {
        dir: dir.to_owned(),
        name,
        deps,
    })
}

/// `a/b/../c` as `a/c`.
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

/// Every path a use tree names: `a::{b, c::{self, d as e}, *}` is `a::b`, `a::c`, `a::c::d`,
/// `a::*`. Aliases are dropped; a leading `::` is ignored.
pub fn expand(tree: &str) -> Vec<Vec<String>> {
    let mut parser = UseTree {
        tokens: tokens(tree),
        position: 0,
        paths: Vec::new(),
    };
    parser.tree(&[]);
    parser.paths
}

fn tokens(text: &str) -> Vec<String> {
    let word_char = |c: char| c.is_alphanumeric() || c == '_' || c == '#';
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ':' if chars.next_if_eq(&':').is_some() => tokens.push("::".to_owned()),
            '{' | '}' | ',' | '*' => tokens.push(c.to_string()),
            c if word_char(c) => {
                let mut word = c.to_string();
                while let Some(next) = chars.next_if(|next| word_char(*next)) {
                    word.push(next);
                }
                tokens.push(word.trim_start_matches("r#").to_owned());
            }
            _ => {}
        }
    }
    tokens
}

/// A use tree's tokens, read from `position`, and the paths read so far.
struct UseTree {
    tokens: Vec<String>,
    position: usize,
    paths: Vec<Vec<String>>,
}

impl UseTree {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.position).map(String::as_str)
    }

    /// One use tree under `prefix`.
    fn tree(&mut self, prefix: &[String]) {
        let mut path = prefix.to_vec();
        loop {
            let token = self.peek().map(str::to_owned);
            match token.as_deref() {
                Some("::") => self.position += 1,
                Some("{") => return self.group(&path),
                None | Some("," | "}") => {
                    if path.len() > prefix.len() {
                        self.paths.push(path);
                    }
                    return;
                }
                Some(segment) => {
                    let at_start = !path.is_empty() && path.len() == prefix.len();
                    if self.segment(&mut path, segment, at_start) {
                        return;
                    }
                }
            }
        }
    }

    /// Reads `segment` onto `path`; true when it ends the path (a glob, a `self` at the start of
    /// a group, or a name no `::` follows), which is then recorded.
    fn segment(&mut self, path: &mut Vec<String>, segment: &str, at_start: bool) -> bool {
        self.position += 1;
        match segment {
            "*" => {
                path.push("*".to_owned());
                self.paths.push(std::mem::take(path));
                true
            }
            "self" if at_start => {
                self.end(std::mem::take(path));
                true
            }
            _ => {
                path.push(segment.to_owned());
                let ends = self.peek() != Some("::");
                if ends {
                    self.end(std::mem::take(path));
                }
                ends
            }
        }
    }

    /// A braced list of use trees under `prefix`, its `{` next.
    fn group(&mut self, prefix: &[String]) {
        self.position += 1;
        while !matches!(self.peek(), Some("}") | None) {
            self.tree(prefix);
            if self.peek() == Some(",") {
                self.position += 1;
            }
        }
        self.position += 1;
    }

    /// `path` complete, past any `as` alias.
    fn end(&mut self, path: Vec<String>) {
        if self.peek() == Some("as") {
            self.position += 2;
        }
        self.paths.push(path);
    }
}

#[cfg(test)]
mod tests {
    use super::{expand, normalize};

    fn paths(tree: &str) -> Vec<String> {
        expand(tree).into_iter().map(|p| p.join("::")).collect()
    }

    #[test]
    fn use_trees_expand_to_every_path_they_name() {
        assert_eq!(paths("std::fmt"), ["std::fmt"]);
        assert_eq!(paths("::std::io::Read"), ["std::io::Read"]);
        assert_eq!(paths("a::b as c"), ["a::b"]);
        assert_eq!(paths("super::*"), ["super::*"]);
        assert_eq!(
            paths("crate::cli::{self, parse as p, nested::{deep, Other}, *}"),
            [
                "crate::cli",
                "crate::cli::parse",
                "crate::cli::nested::deep",
                "crate::cli::nested::Other",
                "crate::cli::*"
            ]
        );
        assert_eq!(paths("{a, b::{c,}}"), ["a", "b::c"]);
        assert_eq!(paths("self::helper"), ["self::helper"]);
    }

    #[test]
    fn paths_normalize_parent_segments() {
        assert_eq!(normalize("crates/app/../core"), "crates/core");
        assert_eq!(normalize("./a/./b"), "a/b");
        assert_eq!(normalize("a/.."), ".");
    }
}
