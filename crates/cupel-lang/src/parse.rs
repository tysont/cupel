// ABOUTME: Parsing and the parse cache: a tree per (content hash, grammar), shared by every producer,
// ABOUTME: so an unchanged file costs one read and one hash. Files that cannot be parsed are problems.
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use cupel_core::LangId;
use tree_sitter::{Node, Parser, Tree};

use crate::language::Language;

/// A parsed source text. Identical texts share one, whatever their paths.
#[derive(Debug)]
pub struct Syntax {
    pub source: String,
    pub tree: Tree,
}

/// One file of a language, parsed: the input to every syntactic producer.
#[derive(Debug, Clone)]
pub struct Parsed {
    /// Relative to the repository root, `/`-separated.
    pub file: String,
    pub lang: LangId,
    pub syntax: Arc<Syntax>,
}

impl Parsed {
    pub fn source(&self) -> &str {
        &self.syntax.source
    }

    pub fn root(&self) -> Node<'_> {
        self.syntax.tree.root_node()
    }

    /// The source text a node spans.
    pub fn text(&self, node: Node<'_>) -> &str {
        &self.syntax.source[node.byte_range()]
    }

    /// The first syntax error in document order: an `ERROR` node or a node the parser had to
    /// invent (`MISSING`). Tree-sitter recovers from errors, so the rest of the tree is usable.
    pub fn first_error(&self) -> Option<Node<'_>> {
        let root = self.root();
        if !root.has_error() {
            return None;
        }
        let mut node = root;
        'descend: loop {
            if node.is_error() || node.is_missing() {
                return Some(node);
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.has_error() {
                    node = child;
                    continue 'descend;
                }
            }
            return Some(node);
        }
    }
}

/// A file that could not be parsed at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseProblem {
    pub file: String,
    pub lang: LangId,
    pub reason: String,
}

/// Cache counters since the cache was created.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
}

type Key = (blake3::Hash, tree_sitter::Language);

/// One cache slot: filled once by whichever caller parses first; `None` when the parser failed.
type Slot = Arc<OnceLock<Option<Arc<Syntax>>>>;

/// Parsed trees keyed by the BLAKE3 hash of the file's content and the grammar that parsed it.
/// In memory for the life of the process; the persistent store arrives with the fact store.
#[derive(Default)]
pub struct ParseCache {
    trees: Mutex<HashMap<Key, Slot>>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl ParseCache {
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }

    /// Reads and parses `file` (relative to `root`) as `language`. Concurrent requests for the
    /// same content wait for one parse.
    pub fn parse(
        &self,
        root: &Path,
        file: &str,
        language: &dyn Language,
    ) -> Result<Parsed, ParseProblem> {
        let problem = |reason: String| ParseProblem {
            file: file.to_owned(),
            lang: language.id(),
            reason,
        };
        let bytes = fs::read(root.join(file)).map_err(|error| problem(error.to_string()))?;
        let source =
            String::from_utf8(bytes).map_err(|_| problem("the file is not UTF-8".to_owned()))?;
        let grammar = language.grammar(Path::new(file));
        let key = (blake3::hash(source.as_bytes()), grammar.clone());
        let cell = Arc::clone(
            self.trees
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(key)
                .or_default(),
        );
        let mut parsed_here = false;
        let syntax = cell.get_or_init(|| {
            parsed_here = true;
            let mut parser = Parser::new();
            parser.set_language(&grammar).ok()?;
            let tree = parser.parse(&source, None)?;
            Some(Arc::new(Syntax { source, tree }))
        });
        let counter = if parsed_here {
            &self.misses
        } else {
            &self.hits
        };
        counter.fetch_add(1, Ordering::Relaxed);
        let syntax = syntax
            .clone()
            .ok_or_else(|| problem("the parser could not run".to_owned()))?;
        Ok(Parsed {
            file: file.to_owned(),
            lang: language.id(),
            syntax,
        })
    }
}
