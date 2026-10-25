// ABOUTME: Parsing and the parse cache: a tree per (content hash, grammar), shared by every producer, so an
// ABOUTME: unchanged file costs one read and one hash; a changed file can be reparsed from its old tree.
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use cupel_core::LangId;
use tree_sitter::{InputEdit, Node, Parser, Point, Tree};

/// A parsed source text. Identical texts share one, whatever their paths.
#[derive(Debug)]
pub struct Syntax {
    pub source: String,
    pub tree: Tree,
    /// What extractors derived from this tree, by file, result type, and extractor: kept with
    /// the tree, so a warm daemon derives nothing again for an unchanged file (ADR-085 §3).
    derived: Mutex<HashMap<DerivedKey, Arc<dyn Any + Send + Sync>>>,
}

/// A derived value's file, result type, and extractor.
type DerivedKey = (String, TypeId, usize);

impl Syntax {
    fn new(source: String, tree: Tree) -> Self {
        Self {
            source,
            tree,
            derived: Mutex::default(),
        }
    }
}

/// One file of a language, parsed: the input to every syntactic producer.
#[derive(Debug, Clone)]
pub struct Parsed {
    /// Relative to the repository root, `/`-separated.
    pub file: String,
    pub lang: LangId,
    /// The grammar the file was parsed with; queries are compiled against it.
    pub grammar: tree_sitter::Language,
    pub syntax: Arc<Syntax>,
}

impl Parsed {
    /// What `derive` makes of this file, made once per file, content, result type, and
    /// `extractor` (an identity for `derive`, such as its function's address) and kept with the
    /// tree. `derive` must read nothing but this file's path, language, and tree.
    pub fn derived<T: Clone + Send + Sync + 'static>(
        &self,
        extractor: usize,
        derive: impl FnOnce() -> T,
    ) -> T {
        let key = (self.file.clone(), TypeId::of::<T>(), extractor);
        let found = lock(&self.syntax.derived)
            .get(&key)
            .and_then(|value| value.downcast_ref::<T>().cloned());
        if let Some(found) = found {
            return found;
        }
        let value = derive();
        lock(&self.syntax.derived).insert(key, Arc::new(value.clone()));
        value
    }

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
    /// An `ERROR` of only tokens directly inside a node of a kind in `token_trees` is a token
    /// the grammar does not know there, not an error (`[syntax]`).
    pub fn first_error(&self, token_trees: &[String]) -> Option<Node<'_>> {
        let root = self.root();
        if !root.has_error() {
            return None;
        }
        first_error_in(root, token_trees)
    }
}

/// The first syntax error at or below `node`, which has one; see [`Parsed::first_error`].
fn first_error_in<'t>(node: Node<'t>, token_trees: &[String]) -> Option<Node<'t>> {
    if node.is_error() || node.is_missing() {
        let mut cursor = node.walk();
        let tolerated = node.is_error()
            && node
                .children(&mut cursor)
                .all(|token| token.child_count() == 0)
            && node
                .parent()
                .is_some_and(|parent| token_trees.iter().any(|kind| kind == parent.kind()));
        return (!tolerated).then_some(node);
    }
    let mut cursor = node.walk();
    let children: Vec<Node<'t>> = node
        .children(&mut cursor)
        .filter(|child| child.has_error())
        .collect();
    if children.is_empty() {
        return Some(node);
    }
    children
        .into_iter()
        .find_map(|child| first_error_in(child, token_trees))
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

/// What [`ParseCache::reparse`] did with a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reparse {
    /// The cache never parsed the file, so it has no tree to start from; the next request parses it.
    Unknown,
    /// The cache already holds a tree for the file's content.
    Unchanged,
    /// The file changed, and its new tree, parsed from the old one, is in the cache.
    Reparsed,
    /// The file is gone or no longer readable text; the next request reports it.
    Gone,
}

/// Parsed trees keyed by the BLAKE3 hash of the file's content and the grammar that parsed it.
/// In memory for the life of the process (the daemon's life under `cupel serve`, ADR-085).
#[derive(Default)]
pub struct ParseCache {
    trees: Mutex<HashMap<Key, Slot>>,
    /// The key each file, by its path under its root, last parsed to: where a reparse starts and
    /// which trees a sweep keeps.
    files: Mutex<HashMap<PathBuf, Key>>,
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

    /// Reads and parses `file` (relative to `root`) of language `lang` with `grammar`.
    /// Concurrent requests for the same content wait for one parse.
    pub fn parse(
        &self,
        root: &Path,
        file: &str,
        lang: &LangId,
        grammar: tree_sitter::Language,
    ) -> Result<Parsed, ParseProblem> {
        let problem = |reason: String| ParseProblem {
            file: file.to_owned(),
            lang: lang.clone(),
            reason,
        };
        let bytes = fs::read(root.join(file)).map_err(|error| problem(error.to_string()))?;
        let source =
            String::from_utf8(bytes).map_err(|_| problem("the file is not UTF-8".to_owned()))?;
        let key = (blake3::hash(source.as_bytes()), grammar.clone());
        let parse_grammar = grammar.clone();
        let cell = Arc::clone(lock(&self.trees).entry(key.clone()).or_default());
        let mut parsed_here = false;
        let syntax = cell.get_or_init(|| {
            parsed_here = true;
            let mut parser = Parser::new();
            parser.set_language(&parse_grammar).ok()?;
            let tree = parser.parse(&source, None)?;
            Some(Arc::new(Syntax::new(source, tree)))
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
        lock(&self.files).insert(root.join(file), key);
        Ok(Parsed {
            file: file.to_owned(),
            lang: lang.clone(),
            grammar,
            syntax,
        })
    }
}

impl ParseCache {
    /// How many trees the cache holds.
    pub fn trees(&self) -> usize {
        lock(&self.trees).len()
    }

    /// Reads `path`, a file this cache parsed before (by its root joined with its relative
    /// path), and when its content changed, parses it again starting from its previous tree
    /// (tree-sitter's incremental parse over the one edit between the two texts), so the next
    /// request finds its tree. The tree equals the one a fresh parse makes.
    pub fn reparse(&self, path: &Path) -> Reparse {
        let Some(old_key) = lock(&self.files).get(path).cloned() else {
            return Reparse::Unknown;
        };
        let Some(source) = fs::read(path)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
        else {
            return Reparse::Gone;
        };
        let grammar = old_key.1.clone();
        let key = (blake3::hash(source.as_bytes()), grammar.clone());
        let (cell, old) = {
            let mut trees = lock(&self.trees);
            let old = trees
                .get(&old_key)
                .and_then(|slot| slot.get().cloned().flatten());
            (Arc::clone(trees.entry(key.clone()).or_default()), old)
        };
        let mut parsed_here = false;
        cell.get_or_init(|| {
            parsed_here = true;
            let mut parser = Parser::new();
            parser.set_language(&grammar).ok()?;
            let old_tree = old.map(|old| {
                let mut tree = old.tree.clone();
                tree.edit(&edit_between(&old.source, &source));
                tree
            });
            let tree = parser.parse(&source, old_tree.as_ref())?;
            Some(Arc::new(Syntax::new(source, tree)))
        });
        lock(&self.files).insert(path.to_path_buf(), key);
        if parsed_here {
            Reparse::Reparsed
        } else {
            Reparse::Unchanged
        }
    }

    /// Forgets the files that no longer exist and drops every tree no remaining file last
    /// parsed to, so a long-lived cache holds about one tree per file.
    pub fn sweep(&self) {
        let mut files = lock(&self.files);
        files.retain(|path, _| path.exists());
        let held: HashSet<&Key> = files.values().collect();
        lock(&self.trees).retain(|key, _| held.contains(key));
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The one edit that turns `old` into `new`: everything between their longest common prefix
/// and their longest common suffix, cut at character boundaries.
fn edit_between(old: &str, new: &str) -> InputEdit {
    let mut prefix = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(prefix) || !new.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let longest = old.len().min(new.len()) - prefix;
    let mut suffix = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take(longest)
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(old.len() - suffix) || !new.is_char_boundary(new.len() - suffix) {
        suffix -= 1;
    }
    let (old_end, new_end) = (old.len() - suffix, new.len() - suffix);
    InputEdit {
        start_byte: prefix,
        old_end_byte: old_end,
        new_end_byte: new_end,
        start_position: point(old, prefix),
        old_end_position: point(old, old_end),
        new_end_position: point(new, new_end),
    }
}

/// The row and byte column of byte `offset`, a character boundary, in `text`.
fn point(text: &str, offset: usize) -> Point {
    let before = &text[..offset];
    let line_start = before.rfind('\n').map_or(0, |at| at + 1);
    Point::new(before.matches('\n').count(), offset - line_start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_edit_between_two_texts_spans_only_what_differs() {
        let edit = edit_between("ab\ncd\nef", "ab\nXYZ\nef");
        assert_eq!(
            (edit.start_byte, edit.old_end_byte, edit.new_end_byte),
            (3, 5, 6)
        );
        assert_eq!(edit.start_position, Point::new(1, 0));
        assert_eq!(edit.old_end_position, Point::new(1, 2));
        assert_eq!(edit.new_end_position, Point::new(1, 3));
        let appended = edit_between("aa", "aaaa");
        assert_eq!(
            (
                appended.start_byte,
                appended.old_end_byte,
                appended.new_end_byte
            ),
            (2, 2, 4)
        );
        // `é` and `è` share their first byte; the edit starts at the character.
        let accented = edit_between("é", "è");
        assert_eq!((accented.start_byte, accented.old_end_byte), (0, 2));
    }
}
