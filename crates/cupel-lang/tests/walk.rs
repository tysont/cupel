// ABOUTME: Tests for the file walk: source files of registered languages only, gitignore-aware
// ABOUTME: even outside a git repository, hidden directories skipped, paths relative and sorted.
use std::fs;
use std::path::Path;

use cupel_lang::{LanguageRegistry, walk};

fn write(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
fn the_walk_returns_exactly_the_source_files_that_are_not_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for path in [
        "src/lib.rs",
        "src/nested/deep/mod.rs",
        "web/app.tsx",
        "web/util.mjs",
        "web/legacy.js",
        "svc/main.go",
        "py/pkg/__init__.py",
        "README.md",
        "notes.txt",
        "target/debug/build.rs",
        "web/node_modules/dep/index.js",
        "web/dist/bundle.js",
        ".hidden/secret.py",
        "py/pkg/generated_pb2.py",
    ] {
        write(root, path, "");
    }
    write(root, ".gitignore", "target/\nnode_modules/\n");
    write(root, "web/.gitignore", "dist/\n");
    write(root, "py/.gitignore", "*_pb2.py\n");

    let files = walk(root, &LanguageRegistry::builtin());

    assert_eq!(
        files,
        [
            "py/pkg/__init__.py",
            "src/lib.rs",
            "src/nested/deep/mod.rs",
            "svc/main.go",
            "web/app.tsx",
            "web/legacy.js",
            "web/util.mjs",
        ]
    );
}

#[test]
fn the_walk_of_an_empty_directory_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    assert!(walk(dir.path(), &LanguageRegistry::builtin()).is_empty());
}
