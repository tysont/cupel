// ABOUTME: Parsing and the parse cache: a tree per (content hash, grammar), shared by every producer, so an
// ABOUTME: unchanged file costs one read and one hash; a changed file can be reparsed from its old tree.
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use cupel_core::LangId;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tree_sitter::{InputEdit, Node, Parser, Point, Tree};

/// Where a parse cache keeps the values it derives beyond its process, and finds them again
/// (ADR-181, ADR-188): each file's values by the file's key ([`ParseCache::shelve`]) and the
/// value's name. A file whose values are all found is never parsed.
pub trait FactShelf: Send + Sync {
    /// The values kept for the file under `key`, by name; empty when none are.
    fn load(&self, key: &str) -> Vec<(String, Vec<u8>)>;
    /// Keeps `payload`, the value `name` derived from the file under `key`.
    fn keep(&self, key: &str, name: &str, payload: Vec<u8>);
    /// Notes that a parse used the file under `key`, whether its values came from the shelf,
    /// from memory, or from its tree, so the store keeps the values of files runs use (ADR-201).
    fn used(&self, key: &str);
}

impl std::fmt::Debug for dyn FactShelf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FactShelf")
    }
}

/// A source text, parsed when first asked. Identical texts share one, whatever their paths.
pub struct Syntax {
    pub source: String,
    grammar: tree_sitter::Language,
    /// The tree, parsed on first use; `None` when the parser could not run.
    tree: OnceLock<Option<Tree>>,
    /// The parse cache's count of trees parsed.
    parses: Arc<AtomicU64>,
    /// What extractors derived from this tree, by file, result type, and extractor: kept with
    /// the tree, so a warm daemon derives nothing again for an unchanged file (ADR-085 §3).
    derived: Mutex<HashMap<DerivedKey, Arc<dyn Any + Send + Sync>>>,
    /// What the cache's shelf held for each file of this text, by file, once loaded.
    shelved: Mutex<HashMap<String, Shelved>>,
}

impl std::fmt::Debug for Syntax {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Syntax")
            .field("source", &self.source)
            .field("tree", &self.tree.get())
            .finish_non_exhaustive()
    }
}

/// What a shelf held for one file: whether it held anything, and the values not read yet, by name.
#[derive(Default)]
struct Shelved {
    found: bool,
    unread: HashMap<String, Vec<u8>>,
}

/// A derived value's file, result type, and extractor name.
type DerivedKey = (String, TypeId, &'static str);

impl Syntax {
    fn new(source: String, grammar: tree_sitter::Language, parses: Arc<AtomicU64>) -> Self {
        Self {
            source,
            grammar,
            tree: OnceLock::new(),
            parses,
            derived: Mutex::default(),
            shelved: Mutex::default(),
        }
    }

    /// The tree, parsing the source on first use; `None` when the parser could not run.
    fn tree(&self) -> Option<&Tree> {
        self.tree
            .get_or_init(|| {
                self.parses.fetch_add(1, Ordering::Relaxed);
                let mut parser = Parser::new();
                parser.set_language(&self.grammar).ok()?;
                parser.parse(&self.source, None)
            })
            .as_ref()
    }

    /// Loads, once per file, what `load` finds on the shelf for `file`: whether it found any,
    /// and whether it loaded now.
    fn shelve(&self, file: &str, load: impl FnOnce() -> Vec<(String, Vec<u8>)>) -> (bool, bool) {
        let mut loaded = false;
        let found = lock(&self.shelved)
            .entry(file.to_owned())
            .or_insert_with(|| {
                loaded = true;
                let unread: HashMap<String, Vec<u8>> = load().into_iter().collect();
                Shelved {
                    found: !unread.is_empty(),
                    unread,
                }
            })
            .found;
        (found, loaded)
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
    /// The cache's shelf and this file's key on it, when the cache has one.
    shelf: Option<(Arc<dyn FactShelf>, Arc<str>)>,
}

impl Parsed {
    /// What `derive` makes of this file, made once per file, content, result type, and
    /// extractor `name` and kept with the tree. With a shelf, a value the shelf holds for this
    /// file is read from it instead, and one derived here is kept on it. `derive` must read
    /// nothing but this file's path, language, and tree; `name` names one extractor, the same
    /// in every build.
    pub fn derived<T>(&self, name: &'static str, derive: impl FnOnce() -> T) -> T
    where
        T: Clone + Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        let key = (self.file.clone(), TypeId::of::<T>(), name);
        let remembered = || {
            lock(&self.syntax.derived)
                .get(&key)
                .and_then(|value| value.downcast_ref::<T>().cloned())
        };
        if let Some(found) = remembered() {
            return found;
        }
        if let Some(found) = self.unshelved::<T>(name) {
            lock(&self.syntax.derived).insert(key.clone(), Arc::new(found.clone()));
            // Removed only once remembered, so a caller that finds it gone finds it remembered.
            if let Some(shelved) = lock(&self.syntax.shelved).get_mut(&self.file) {
                shelved.unread.remove(name);
            }
            return found;
        }
        if let Some(found) = remembered() {
            return found;
        }
        let value = derive();
        if let Some((shelf, file_key)) = &self.shelf
            && let Ok(payload) = serde_json::to_vec(&value)
        {
            shelf.keep(file_key, name, payload);
        }
        lock(&self.syntax.derived).insert(key, Arc::new(value.clone()));
        value
    }

    /// The value `name` the shelf holds for this file, when it holds one that reads back.
    fn unshelved<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        let payload = lock(&self.syntax.shelved)
            .get(&self.file)?
            .unread
            .get(name)?
            .clone();
        serde_json::from_slice(&payload).ok()
    }

    pub fn source(&self) -> &str {
        &self.syntax.source
    }

    /// The root of the file's tree, which is parsed now if it was not yet.
    ///
    /// # Panics
    ///
    /// When the parser cannot run on a text whose values the shelf held, which it parsed before.
    pub fn root(&self) -> Node<'_> {
        self.syntax
            .tree()
            .expect("a text the cache accepted parses")
            .root_node()
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
    let children: Vec<Node<'t>> = node.children(&mut cursor).filter(Node::has_error).collect();
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

/// One cache slot: filled once by whichever caller reads the content first.
type Slot = Arc<OnceLock<Arc<Syntax>>>;

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
    /// Where derived values are kept beyond the process, once a run gives the cache one.
    shelf: OnceLock<Arc<dyn FactShelf>>,
    hits: AtomicU64,
    misses: AtomicU64,
    parses: Arc<AtomicU64>,
    shelved: AtomicU64,
}

impl ParseCache {
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }

    /// How many trees the cache has parsed: a file whose derived values all came from the shelf
    /// needs none.
    pub fn trees_parsed(&self) -> u64 {
        self.parses.load(Ordering::Relaxed)
    }

    /// How many files the cache found values for on the shelf.
    pub fn files_shelved(&self) -> u64 {
        self.shelved.load(Ordering::Relaxed)
    }

    /// Gives the cache `given`, the shelf where every value a [`Parsed::derived`] makes is kept
    /// and found again; the first shelf given stays. Whether this one was taken.
    pub fn shelve(&self, given: Arc<dyn FactShelf>) -> bool {
        self.shelf.set(given).is_ok()
    }

    /// Reads and parses `file` (relative to `root`) of language `lang` with `grammar`.
    /// Concurrent requests for the same content wait for one parse. With a shelf that holds
    /// values for the file, the tree is parsed only when something asks for it.
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
        let hash = blake3::hash(source.as_bytes());
        let key = (hash, grammar.clone());
        let cell = Arc::clone(lock(&self.trees).entry(key.clone()).or_default());
        let mut read_here = false;
        let syntax = Arc::clone(cell.get_or_init(|| {
            read_here = true;
            Arc::new(Syntax::new(
                source,
                grammar.clone(),
                Arc::clone(&self.parses),
            ))
        }));
        let counter = if read_here { &self.misses } else { &self.hits };
        counter.fetch_add(1, Ordering::Relaxed);
        let kept_on = self.shelf.get().map(|given| {
            let file_key: Arc<str> = shelf_key(file, &hash, lang, &grammar).into();
            (Arc::clone(given), file_key)
        });
        let (on_shelf, loaded) = kept_on
            .as_ref()
            .map_or((false, false), |(given, file_key)| {
                given.used(file_key);
                syntax.shelve(file, || given.load(file_key))
            });
        if on_shelf && loaded {
            self.shelved.fetch_add(1, Ordering::Relaxed);
        }
        if !on_shelf && syntax.tree().is_none() {
            return Err(problem("the parser could not run".to_owned()));
        }
        lock(&self.files).insert(root.join(file), key);
        Ok(Parsed {
            file: file.to_owned(),
            lang: lang.clone(),
            grammar,
            syntax,
            shelf: kept_on,
        })
    }
}

/// The key of `file`'s values on a shelf: BLAKE3 of the NUL-separated path, content hash,
/// language, and grammar (its name, version, and ABI). The shelf adds the build (ADR-188).
fn shelf_key(
    file: &str,
    hash: &blake3::Hash,
    lang: &LangId,
    grammar: &tree_sitter::Language,
) -> String {
    let version = grammar
        .metadata()
        .map(|version| {
            format!(
                "{}.{}.{}",
                version.major_version, version.minor_version, version.patch_version
            )
        })
        .unwrap_or_default();
    let abi = grammar.abi_version().to_string();
    let hash = hash.to_hex();
    let parts = [
        file,
        hash.as_str(),
        lang.as_str(),
        grammar.name().unwrap_or_default(),
        version.as_str(),
        abi.as_str(),
    ];
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update(&[0]);
    }
    hasher.finalize().to_hex().to_string()
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
            let old = trees.get(&old_key).and_then(|slot| slot.get().cloned());
            (Arc::clone(trees.entry(key.clone()).or_default()), old)
        };
        let mut parsed_here = false;
        let syntax = cell.get_or_init(|| {
            parsed_here = true;
            Arc::new(Syntax::new(
                source,
                grammar.clone(),
                Arc::clone(&self.parses),
            ))
        });
        if parsed_here {
            let old_tree = old.as_ref().and_then(|old| {
                let mut tree = old.tree.get()?.as_ref()?.clone();
                tree.edit(&edit_between(&old.source, &syntax.source));
                Some(tree)
            });
            syntax.tree.get_or_init(|| {
                self.parses.fetch_add(1, Ordering::Relaxed);
                let mut parser = Parser::new();
                parser.set_language(&grammar).ok()?;
                parser.parse(&syntax.source, old_tree.as_ref())
            });
        }
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
