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
//!   (`from pkg import a, b`), as the module spells it (not a local alias).
//! - `@import.alias` (optional) on the local name the match's `@import.name` is bound under
//!   (`from pkg import a as b`): an alias (ADR-141).
//! - `@import.default` (optional) on the local name of a default import (`import d from`): the
//!   statement imports the name `default`, bound under that local name.
//! - `@import.names` (optional) on a list whose named children each import one name (`{ a, b as
//!   c }`): each child's `name` field, or the child itself when it has none, as written; a
//!   child's `alias` field is the local name it is bound under, unless the statement is a
//!   re-export, which binds nothing locally.
//! - `@import.module` (optional) marks a declaration of a child module whose file sits next to
//!   the declaring file (Rust `mod a;`): an edge of kind `declaration`.
//! - `@import.reexport` (optional) marks a statement that exports what it imports (Rust
//!   `pub use`, `export … from`): an edge of kind `re-export`.
//! - `@reexport.name` (any pattern, no `@import` needed) on a name the file exports (an entry of
//!   Python's `__all__`): an import whose `@import.name` is such a name is a `re-export`.
//!   Every other import is an `import` edge (ADR-068).
//! - `@scope` + `@scope.name` (optional) on inline modules, so the resolver knows which module
//!   an import inside one is written in.
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use cupel_core::syntax::{EdgeKind, ImportAlias, ModuleGraph};
use cupel_core::{
    Availability, Cancellation, CostClass, Ctx, EventSink, FactKind, Langs, ProduceCtx, Producer,
    ProducerId, Result, Scope,
};
use globset::Glob;

use crate::contract::capture;
use crate::language::{Language, owns, parse_one, producer_id};
use crate::parse::{ParseCache, Parsed};
use crate::producer::parse_scope;
use crate::query::{Match, encloses, matches};
use crate::spec::QueryFile;

/// One import as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawImport {
    pub specifier: String,
    pub names: Vec<String>,
    /// The names of `names` the statement binds under another local name.
    pub aliases: Vec<ImportAlias>,
    /// What the statement does with the module: a use, a child-module declaration (`mod a;`),
    /// or a re-export.
    pub kind: EdgeKind,
    /// The inline modules the import is written in, outermost first.
    pub scope: Vec<String>,
    /// The file a module declaration names itself (`@import.path`, Rust's `#[path = "…"]`),
    /// relative to the declaring file's directory; `None` for the language's default.
    pub path: Option<String>,
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
    let reexported: BTreeSet<&str> = found
        .iter()
        .flat_map(|m| m.nodes(capture::REEXPORT_NAME))
        .map(|node| unquote(parsed.text(node)))
        .collect();
    let mut imports: Vec<RawImport> = found
        .iter()
        .filter_map(|m| {
            let statement = m.node(capture::IMPORT)?;
            let specifier = m.node(capture::IMPORT_SPECIFIER)?;
            let names = imported(parsed, m);
            Some(RawImport {
                specifier: unquote(parsed.text(specifier)).to_owned(),
                kind: edge_kind(m, &names, &reexported),
                aliases: aliases(parsed, m),
                names,
                scope: path_of(statement),
                path: m
                    .node(capture::IMPORT_PATH)
                    .map(|path| unquote(parsed.text(path)).to_owned()),
                line: crate::query::one_based(statement.start_position().row),
            })
        })
        .collect();
    // A statement one pattern matched with its `@import.path` and another without is one
    // import, the one that names its file.
    let named: BTreeSet<(u32, String)> = imports
        .iter()
        .filter(|import| import.path.is_some())
        .map(|import| (import.line, import.specifier.clone()))
        .collect();
    imports.retain(|import| {
        import.path.is_some() || !named.contains(&(import.line, import.specifier.clone()))
    });
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

/// The names the import `m` matched takes from its module: `default` for a default import, then
/// each `@import.name`, then each name an `@import.names` list holds.
fn imported(parsed: &Parsed, m: &Match<'_, '_>) -> Vec<String> {
    m.has(capture::IMPORT_DEFAULT)
        .then(|| "default".to_owned())
        .into_iter()
        .chain(
            m.nodes(capture::IMPORT_NAME)
                .map(|n| parsed.text(n).to_owned()),
        )
        .chain(
            m.nodes(capture::IMPORT_NAMES)
                .flat_map(|list| listed(parsed, list)),
        )
        .collect()
}

/// The names a list of imported names holds: each named child's `name` field, or the child.
fn listed(parsed: &Parsed, list: tree_sitter::Node<'_>) -> Vec<String> {
    let mut cursor = list.walk();
    list.named_children(&mut cursor)
        .filter(|child| !child.is_extra())
        .map(|child| {
            let name = child.child_by_field_name("name").unwrap_or(child);
            parsed.text(name).to_owned()
        })
        .collect()
}

/// The names the import `m` matched binds under another local name: a default import's local
/// name, the `@import.alias` of its `@import.name`, and each `alias` field of an `@import.names`
/// list that is not re-exported.
fn aliases(parsed: &Parsed, m: &Match<'_, '_>) -> Vec<ImportAlias> {
    let alias = |name: &str, local: tree_sitter::Node<'_>| ImportAlias {
        name: name.to_owned(),
        local: parsed.text(local).to_owned(),
    };
    let default = m
        .node(capture::IMPORT_DEFAULT)
        .map(|local| alias("default", local));
    let named = m.node(capture::IMPORT_NAME).and_then(|name| {
        m.node(capture::IMPORT_ALIAS)
            .map(|local| alias(parsed.text(name), local))
    });
    let listed = m
        .nodes(capture::IMPORT_NAMES)
        .filter(|_| !m.has(capture::IMPORT_REEXPORT))
        .flat_map(|list| {
            let mut cursor = list.walk();
            list.named_children(&mut cursor)
                .filter_map(|child| {
                    let name = child.child_by_field_name("name")?;
                    let local = child.child_by_field_name("alias")?;
                    Some(alias(parsed.text(name), local))
                })
                .collect::<Vec<_>>()
        });
    default.into_iter().chain(named).chain(listed).collect()
}

/// What the import `m` matched does with its module: declares it, re-exports it (by its
/// statement, or because a name it imports is in `reexported`), or uses it.
fn edge_kind(m: &Match<'_, '_>, names: &[String], reexported: &BTreeSet<&str>) -> EdgeKind {
    if m.has(capture::IMPORT_MODULE) {
        EdgeKind::Declaration
    } else if m.has(capture::IMPORT_REEXPORT)
        || names.iter().any(|name| reexported.contains(name.as_str()))
    {
        EdgeKind::ReExport
    } else {
        EdgeKind::Import
    }
}

/// `text` without one pair of surrounding quotes.
pub(crate) fn unquote(text: &str) -> &str {
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

impl<'a> Imports<'a> {
    /// What a resolver of `language` sees when it resolves the imports of `scope`.
    pub(crate) fn new(
        root: &'a Path,
        language: &'a dyn Language,
        cache: &'a ParseCache,
        scope: Vec<String>,
    ) -> Self {
        Self {
            root,
            language,
            cache,
            scope,
            listings: Mutex::new(HashMap::new()),
        }
    }
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

    /// The directories below `base` a workspace member pattern (Cargo `members`, uv
    /// `members`, npm `workspaces`) names, in sorted order. Segments containing `*`, `?`, or
    /// `[…]` are globs matching one directory name; other segments name a directory exactly.
    pub fn expand(&self, base: &str, pattern: &str) -> Vec<String> {
        let mut dirs = vec![base.to_owned()];
        for segment in pattern.split('/').filter(|s| !s.is_empty() && *s != ".") {
            let glob = segment
                .contains(['*', '?', '['])
                .then(|| Glob::new(segment).ok().map(|glob| glob.compile_matcher()));
            dirs = dirs
                .iter()
                .flat_map(|dir| match &glob {
                    Some(Some(matcher)) => self
                        .listing(dir)
                        .dirs
                        .iter()
                        .filter(|name| matcher.is_match(name.as_str()))
                        .map(|name| join(dir, name))
                        .collect(),
                    Some(None) => Vec::new(),
                    None => {
                        let child = join(dir, segment);
                        if self.is_dir(&child) {
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
        .map(|parsed| {
            let extractor = file_imports as fn(&Parsed, &dyn Language) -> FileImports as usize;
            parsed.derived(extractor, || file_imports(&parsed, self.language))
        })
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
        let imports = Imports::new(
            &ctx.ctx.root,
            self.language.as_ref(),
            &self.cache,
            scope
                .files
                .iter()
                .filter(|file| owns(self.language.as_ref(), file))
                .cloned()
                .collect(),
        );
        let graph = (self.resolve)(&imports);
        ctx.cancel.checkpoint()?;
        Ok(graph)
    }
}
