// ABOUTME: Tests for the parse cache: keyed by content hash and grammar, so a second parse of an
// ABOUTME: unchanged tree is all hits, identical files share a tree, a changed file reparses incrementally, and sweeps.
use std::fs;

use cupel_core::Cancellation;
use cupel_core::syntax::ProjectInfo;
use cupel_lang::{CacheStats, LanguageRegistry, Reparse, walk};

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

/// The tree of `file` under `dir` as a fresh registry parses it: its S-expression and every
/// node's byte range, in document order.
fn cold_tree(dir: &std::path::Path, file: &str) -> (String, Vec<(usize, usize)>) {
    let registry = LanguageRegistry::builtin();
    let parsed = registry.parse_all(dir, &[file.to_owned()], &Cancellation::new());
    shape(parsed[0].as_ref().unwrap())
}

fn shape(parsed: &cupel_lang::Parsed) -> (String, Vec<(usize, usize)>) {
    let mut ranges = Vec::new();
    let mut cursor = parsed.root().walk();
    let mut descend = true;
    loop {
        if descend {
            let node = cursor.node();
            ranges.push((node.start_byte(), node.end_byte()));
        }
        if descend && cursor.goto_first_child() {
            continue;
        }
        if cursor.goto_next_sibling() {
            descend = true;
            continue;
        }
        if !cursor.goto_parent() {
            break;
        }
        descend = false;
    }
    (parsed.root().to_sexp(), ranges)
}

#[test]
fn a_changed_file_reparses_from_its_old_tree_to_the_tree_a_cold_parse_makes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let before = "fn a(x: i32) -> i32 {\n    if x > 1 { x } else { 0 }\n}\n\nfn b() {}\n";
    fs::write(root.join("a.rs"), before).unwrap();
    let registry = LanguageRegistry::builtin();
    let files = vec!["a.rs".to_owned()];
    let cancel = Cancellation::new();
    registry.parse_all(&root, &files, &cancel);

    // Edits in the middle, at the start, and at the end, one after another.
    let edits = [
        "fn a(x: i32) -> i32 {\n    if x >= 10 && x < 20 { x * 2 } else { 0 }\n}\n\nfn b() {}\n",
        "// lead\nfn a(x: i32) -> i32 {\n    if x >= 10 && x < 20 { x * 2 } else { 0 }\n}\n\nfn b() {}\n",
        "// lead\nfn a(x: i32) -> i32 {\n    if x >= 10 && x < 20 { x * 2 } else { 0 }\n}\n\nfn b() { let é = 1; }\nfn c() {}\n",
    ];
    for (round, text) in edits.iter().enumerate() {
        fs::write(root.join("a.rs"), text).unwrap();
        assert_eq!(
            registry.cache().reparse(&root.join("a.rs")),
            Reparse::Reparsed,
            "edit {round}"
        );
        let stats = registry.cache().stats();
        let parsed = registry.parse_all(&root, &files, &cancel);
        assert_eq!(
            registry.cache().stats(),
            CacheStats {
                hits: stats.hits + 1,
                misses: stats.misses
            },
            "edit {round}: the request after a reparse finds the tree"
        );
        assert_eq!(
            shape(parsed[0].as_ref().unwrap()),
            cold_tree(&root, "a.rs"),
            "edit {round}"
        );
    }
}

#[test]
fn reparse_leaves_alone_files_never_parsed_unchanged_files_and_files_gone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    fs::write(root.join("a.py"), "def a():\n    pass\n").unwrap();
    fs::write(root.join("b.py"), "def b():\n    pass\n").unwrap();
    let registry = LanguageRegistry::builtin();
    registry.parse_all(&root, &["a.py".to_owned()], &Cancellation::new());
    let cache = registry.cache();

    assert_eq!(cache.reparse(&root.join("b.py")), Reparse::Unknown);
    assert_eq!(cache.reparse(&root.join("a.py")), Reparse::Unchanged);
    fs::remove_file(root.join("a.py")).unwrap();
    assert_eq!(cache.reparse(&root.join("a.py")), Reparse::Gone);
    assert_eq!(cache.stats(), CacheStats { hits: 0, misses: 1 });
}

#[test]
fn sweep_keeps_the_trees_of_files_that_still_hold_them_and_drops_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    fs::write(root.join("a.go"), "package a\n").unwrap();
    fs::write(root.join("b.go"), "package b\n").unwrap();
    fs::write(root.join("c.go"), "package a\n").unwrap();
    let files: Vec<String> = ["a.go", "b.go", "c.go"].map(str::to_owned).to_vec();
    let registry = LanguageRegistry::builtin();
    let cancel = Cancellation::new();
    registry.parse_all(&root, &files, &cancel);
    assert_eq!(registry.cache().trees(), 2, "a.go and c.go share one tree");

    // b.go changes (its old tree goes), c.go is removed (its tree stays: a.go holds it).
    fs::write(root.join("b.go"), "package b2\n").unwrap();
    registry.parse_all(&root, &files[..2], &cancel);
    fs::remove_file(root.join("c.go")).unwrap();
    registry.cache().sweep();
    assert_eq!(registry.cache().trees(), 2);

    fs::remove_file(root.join("a.go")).unwrap();
    registry.cache().sweep();
    assert_eq!(registry.cache().trees(), 1, "only b.go's tree is left");
    let stats = registry.cache().stats();
    registry.parse_all(&root, &files[1..2], &cancel);
    assert_eq!(registry.cache().stats().hits, stats.hits + 1);
}
