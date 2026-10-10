// ABOUTME: `QueryImports`: raw import statements from imports.scm, and the `ModuleGraph` producer that
// ABOUTME: hands them to a language's hand-written resolver (module resolution is not a query's job).
//!
//! # `imports.scm`
//!
//! - `@import` on the import statement (its line is the edge's line).
//! - `@import.specifier` on the node whose text (quotes removed) is what is imported: a path
//!   (`./b`, `a.b.c`), a module name, or a language-specific tree the resolver expands (a Rust
//!   use tree such as `crate::a::{b, c}`). A statement importing several modules matches once
//!   per specifier.
//! - `@import.name` (optional, repeatable) on each name the statement imports from the module
//!   (`from pkg import a, b`).
//! - `@import.module` (optional) marks a declaration of a child module whose file sits next to
//!   the declaring file (Rust `mod a;`).
//! - `@scope` + `@scope.name` (optional) on inline modules, so the resolver knows which module
//!   an import inside one is written in.
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use cupel_core::syntax::ModuleGraph;
use cupel_core::{
    Availability, Cancellation, CostClass, Ctx, EventSink, FactKind, Langs, ProduceCtx, Producer,
    ProducerId, Result, Scope,
};

use crate::contract::capture;
use crate::language::{Language, owns, parse_one};
use crate::parse::{ParseCache, Parsed};
use crate::producer::{parse_scope, producer_id};
use crate::query::{encloses, matches};
use crate::spec::QueryFile;

/// One import as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawImport {
    pub specifier: String,
    pub names: Vec<String>,
    /// A child-module declaration (`mod a;`) rather than an import.
    pub module: bool,
    /// The inline modules the import is written in, outermost first.
    pub scope: Vec<String>,
    pub line: u32,
}

/// A file's imports and the inline modules it declares (each as its path within the file).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileImports {
    pub imports: Vec<RawImport>,
    pub modules: Vec<Vec<String>>,
}

/// The imports of one parsed file, in document order.
///
/// # Panics
///
/// When the language's `imports.scm` does not compile; a test compiles every query.
pub fn file_imports(parsed: &Parsed, language: &dyn Language) -> FileImports {
    let query = language
        .spec()
        .query(QueryFile::Imports, &parsed.grammar)
        .unwrap_or_else(|error| panic!("{} imports.scm: {error}", language.id()));
    let found = matches(&query, parsed);
    let mut scopes: Vec<(tree_sitter::Node<'_>, &str)> = found
        .iter()
        .filter_map(|m| {
            Some((
                m.node(capture::SCOPE)?,
                parsed.text(m.node(capture::SCOPE_NAME)?),
            ))
        })
        .collect();
    scopes.sort_by_key(|(node, _)| node.start_byte());
    scopes.dedup_by_key(|(node, _)| node.id());
    let path_of = |node: tree_sitter::Node<'_>| -> Vec<String> {
        scopes
            .iter()
            .filter(|(scope, _)| encloses(*scope, node))
            .map(|(_, name)| (*name).to_owned())
            .collect()
    };
    let mut imports: Vec<RawImport> = found
        .iter()
        .filter_map(|m| {
            let statement = m.node(capture::IMPORT)?;
            let specifier = m.node(capture::IMPORT_SPECIFIER)?;
            Some(RawImport {
                specifier: unquote(parsed.text(specifier)).to_owned(),
                names: m
                    .nodes(capture::IMPORT_NAME)
                    .map(|n| parsed.text(n).to_owned())
                    .collect(),
                module: m.has(capture::IMPORT_MODULE),
                scope: path_of(statement),
                line: crate::query::one_based(statement.start_position().row),
            })
        })
        .collect();
    imports.dedup();
    let modules = scopes
        .iter()
        .map(|(node, name)| {
            let mut path = path_of(*node);
            path.push((*name).to_owned());
            path
        })
        .collect();
    FileImports { imports, modules }
}

fn unquote(text: &str) -> &str {
    for quote in ['"', '\'', '`'] {
        if let Some(inner) = text.strip_prefix(quote).and_then(|t| t.strip_suffix(quote)) {
            return inner;
        }
    }
    text
}

/// What a resolver may look at: the scope, and any file of the language under the root
/// (module trees reach files outside the scope).
///
/// Paths are relative to the root and compare exactly, by the names directories list, so
/// `src/Account.rs` does not exist beside `src/account.rs` even on a file system that ignores
/// case: module systems and other machines do not. Listings are read once per directory.
pub struct Imports<'a> {
    root: &'a Path,
    language: &'a dyn Language,
    cache: &'a ParseCache,
    scope: Vec<String>,
    listings: Mutex<HashMap<String, Arc<Listing>>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Entry {
    File,
    Dir,
}

/// The names of one directory's files and subdirectories (symbolic links followed).
#[derive(Default)]
struct Listing {
    files: BTreeSet<String>,
    dirs: BTreeSet<String>,
}

impl Imports<'_> {
    pub fn root(&self) -> &Path {
        self.root
    }

    /// The language's files in scope, in scope order.
    pub fn scope(&self) -> &[String] {
        &self.scope
    }

    /// Whether `path` names a file, every component spelled as its directory lists it.
    pub fn is_file(&self, path: &str) -> bool {
        self.exists(path, Entry::File)
    }

    /// Whether `path` names a directory, every component spelled as its directory lists it.
    /// The root (`.` or empty) is one.
    pub fn is_dir(&self, path: &str) -> bool {
        self.exists(path, Entry::Dir)
    }

    pub fn read(&self, path: &str) -> Option<String> {
        if !self.is_file(path) {
            return None;
        }
        fs::read_to_string(self.root.join(path)).ok()
    }

    /// Subdirectories of `dir` (relative to the root), sorted.
    pub fn dirs_in(&self, dir: &str) -> Vec<String> {
        self.listing(dir)
            .dirs
            .iter()
            .map(|name| join(dir, name))
            .collect()
    }

    /// Files directly in `dir` with `extension` (such as `.rs`), sorted.
    pub fn files_in(&self, dir: &str, extension: &str) -> Vec<String> {
        self.listing(dir)
            .files
            .iter()
            .filter(|name| name.ends_with(extension))
            .map(|name| join(dir, name))
            .collect()
    }

    /// Whether `path` is an entry of kind `entry`, listed by exactly that name in its parent,
    /// below directories each listed by exactly their names.
    fn exists(&self, path: &str, entry: Entry) -> bool {
        let Some(mut parts) = components(path) else {
            return false;
        };
        let Some(last) = parts.pop() else {
            return entry == Entry::Dir;
        };
        let mut dir = String::from(".");
        for part in parts {
            if !self.listing(&dir).dirs.contains(&part) {
                return false;
            }
            dir = join(&dir, &part);
        }
        let listing = self.listing(&dir);
        match entry {
            Entry::File => listing.files.contains(&last),
            Entry::Dir => listing.dirs.contains(&last),
        }
    }

    /// The listing of `dir`; empty when it is not a directory spelled exactly so.
    fn listing(&self, dir: &str) -> Arc<Listing> {
        let key = components(dir).map_or_else(String::new, |parts| parts.join("/"));
        if let Some(listing) = self
            .listings
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return Arc::clone(listing);
        }
        let exact = key.is_empty() || self.is_dir(&key);
        let listing = Arc::new(if exact {
            Listing::read(&self.root.join(&key))
        } else {
            Listing::default()
        });
        self.listings
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, Arc::clone(&listing));
        listing
    }

    /// The imports of `path`, parsed through the cache; empty when it cannot be parsed.
    pub fn of(&self, path: &str) -> FileImports {
        parse_one(
            self.cache,
            self.root,
            path,
            self.language,
            &Cancellation::new(),
        )
        .map(|parsed| file_imports(&parsed, self.language))
        .unwrap_or_default()
    }
}

impl Listing {
    fn read(dir: &Path) -> Self {
        let mut listing = Self::default();
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(kind) = fs::metadata(entry.path()).map(|meta| meta.file_type()) else {
                continue;
            };
            if kind.is_dir() {
                listing.dirs.insert(name);
            } else if kind.is_file() {
                listing.files.insert(name);
            }
        }
        listing
    }
}

/// The components of a root-relative path, `.` segments dropped and `..` applied; `None` when
/// it climbs above the root.
fn components(path: &str) -> Option<Vec<String>> {
    let mut parts: Vec<String> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part.to_owned()),
        }
    }
    Some(parts)
}

/// `dir/name`, or `name` at the root.
pub fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() || dir == "." {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

/// A language's hand-written module resolution.
pub type Resolve = fn(&Imports<'_>) -> ModuleGraph;

/// The `ModuleGraph` producer: parses the scope (reporting broken files), then resolves.
pub struct ImportGraph {
    language: Arc<dyn Language>,
    cache: Arc<ParseCache>,
    resolve: Resolve,
}

impl ImportGraph {
    pub fn new(language: Arc<dyn Language>, cache: Arc<ParseCache>, resolve: Resolve) -> Self {
        Self {
            language,
            cache,
            resolve,
        }
    }
}

impl Producer<ModuleGraph> for ImportGraph {
    fn id(&self) -> ProducerId {
        producer_id(self.language.as_ref())
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![self.language.id()])
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn cost(&self) -> CostClass {
        CostClass::Quick
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        sink: &mut dyn EventSink,
    ) -> Result<ModuleGraph> {
        parse_scope(&self.cache, ctx, scope, self.language.as_ref(), sink)?;
        let imports = Imports {
            root: &ctx.ctx.root,
            language: self.language.as_ref(),
            cache: &self.cache,
            scope: scope
                .files
                .iter()
                .filter(|file| owns(self.language.as_ref(), file))
                .cloned()
                .collect(),
            listings: Mutex::new(HashMap::new()),
        };
        let graph = (self.resolve)(&imports);
        ctx.cancel.checkpoint()?;
        Ok(graph)
    }
}
