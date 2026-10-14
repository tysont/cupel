// ABOUTME: Tests for the parse cache: keyed by content hash and grammar, so a second parse of an
// ABOUTME: unchanged tree is all hits, an edited file misses once, and identical files share a tree.
use std::fs;

use cupel_core::Cancellation;
use cupel_core::syntax::ProjectInfo;
use cupel_lang::{CacheStats, LanguageRegistry, walk};

#[test]
fn a_second_parse_of_an_unchanged_tree_hits_the_cache_for_every_file() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.rs"), "fn a() {}\n").unwrap();
    fs::write(dir.path().join("src/b.rs"), "fn b() {}\n").unwrap();
    fs::write(dir.path().join("c.py"), "def c():\n    pass\n").unwrap();
    let registry = LanguageRegistry::builtin();
    let files = walk(dir.path(), &registry, &ProjectInfo::default());
    let cancel = Cancellation::new();

    let first = registry.parse_all(dir.path(), &files, &cancel);
    assert_eq!(registry.cache().stats(), CacheStats { hits: 0, misses: 3 });
    let second = registry.parse_all(dir.path(), &files, &cancel);
    assert_eq!(registry.cache().stats(), CacheStats { hits: 3, misses: 3 });

    assert_eq!(first.len(), 3);
    assert_eq!(second.len(), 3);
    let shared: Vec<(&str, bool)> = first
        .iter()
        .zip(&second)
        .map(|(a, b)| {
            let (a, b) = (a.as_ref().unwrap(), b.as_ref().unwrap());
            assert_eq!(a.file, b.file);
            (
                a.file.as_str(),
                std::sync::Arc::ptr_eq(&a.syntax, &b.syntax),
            )
        })
        .collect();
    assert_eq!(
        shared,
        [("c.py", true), ("src/a.rs", true), ("src/b.rs", true)],
        "each file's second parse is its first tree"
    );
}

#[test]
fn an_edited_file_misses_once_and_identical_files_share_one_tree() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    fs::write(dir.path().join("b.rs"), "fn a() {}\n").unwrap();
    let registry = LanguageRegistry::builtin();
    let files = vec!["a.rs".to_owned(), "b.rs".to_owned()];
    let cancel = Cancellation::new();

    let parsed = registry.parse_all(dir.path(), &files, &cancel);
    let stats = registry.cache().stats();
    assert_eq!(stats.hits + stats.misses, 2);
    assert_eq!(stats.misses, 1, "identical content parses once");
    assert_eq!(parsed[1].as_ref().unwrap().file, "b.rs");

    fs::write(dir.path().join("a.rs"), "fn a() { 1 }\n").unwrap();
    registry.parse_all(dir.path(), &files, &cancel);
    assert_eq!(registry.cache().stats(), CacheStats { hits: 2, misses: 2 });
}
