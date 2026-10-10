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
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::Arc;

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
pub struct Imports<'a> {
    root: &'a Path,
    language: &'a dyn Language,
    cache: &'a ParseCache,
    scope: Vec<String>,
}

impl Imports<'_> {
    pub fn root(&self) -> &Path {
        self.root
    }

    /// The language's files in scope, in scope order.
    pub fn scope(&self) -> &[String] {
        &self.scope
    }

    pub fn is_file(&self, path: &str) -> bool {
        self.root.join(path).is_file()
    }

    pub fn is_dir(&self, path: &str) -> bool {
        self.root.join(path).is_dir()
    }

    pub fn read(&self, path: &str) -> Option<String> {
        fs::read_to_string(self.root.join(path)).ok()
    }

    /// Subdirectories of `dir` (relative to the root), sorted.
    pub fn dirs_in(&self, dir: &str) -> Vec<String> {
        let mut found: BTreeSet<String> = BTreeSet::new();
        if let Ok(entries) = fs::read_dir(self.root.join(dir)) {
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|t| t.is_dir())
                    && let Some(name) = entry.file_name().to_str()
                {
                    found.insert(join(dir, name));
                }
            }
        }
        found.into_iter().collect()
    }

    /// Files directly in `dir` with `extension` (such as `.rs`), sorted.
    pub fn files_in(&self, dir: &str, extension: &str) -> Vec<String> {
        let mut found: BTreeSet<String> = BTreeSet::new();
        if let Ok(entries) = fs::read_dir(self.root.join(dir)) {
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|t| t.is_file())
                    && let Some(name) = entry.file_name().to_str()
                    && name.ends_with(extension)
                {
                    found.insert(join(dir, name));
                }
            }
        }
        found.into_iter().collect()
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
        };
        let graph = (self.resolve)(&imports);
        ctx.cancel.checkpoint()?;
        Ok(graph)
    }
}
