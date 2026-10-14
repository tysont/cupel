// ABOUTME: Tests for the file walk: source files of registered languages only, gitignore-aware even
// ABOUTME: outside a git repository, hidden and project-generated directories skipped, paths sorted.
use std::fs;
use std::path::Path;

use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{Exclude, LangId};
use cupel_lang::{LanguageRegistry, walk, walk_excluding};

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

    let files = walk(root, &LanguageRegistry::builtin(), &ProjectInfo::default());

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
    assert!(
        walk(
            dir.path(),
            &LanguageRegistry::builtin(),
            &ProjectInfo::default()
        )
        .is_empty()
    );
}

fn project_root(path: &str, skip_dirs: &[&str]) -> ProjectRoot {
    ProjectRoot {
        path: path.to_owned(),
        lang: LangId::from("javascript"),
        build_files: Vec::new(),
        members: Vec::new(),
        packages: Vec::new(),
        skip_dirs: skip_dirs.iter().map(|dir| (*dir).to_owned()).collect(),
    }
}

#[test]
fn the_walk_skips_each_project_roots_generated_directories_below_that_root_only() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for path in [
        "web/src/app.js",
        "web/dist/bundle.js",
        "web/packages/ui/dist/ui.js",
        "tools/dist/script.js",
        "svc/vendor/dep/dep.go",
        "svc/main.go",
    ] {
        write(root, path, "");
    }
    let project = ProjectInfo {
        roots: vec![
            project_root("web", &["dist"]),
            project_root("svc", &["vendor"]),
        ],
    };

    let files = walk(root, &LanguageRegistry::builtin(), &project);

    assert_eq!(
        files,
        ["svc/main.go", "tools/dist/script.js", "web/src/app.js"]
    );
}

#[test]
fn a_project_root_at_the_top_skips_its_generated_directories_everywhere() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "src/lib.rs", "");
    write(dir.path(), "crates/a/target/gen.rs", "");
    let project = ProjectInfo {
        roots: vec![project_root(".", &["target"])],
    };
    assert_eq!(
        walk(dir.path(), &LanguageRegistry::builtin(), &project),
        ["src/lib.rs"]
    );
}

#[test]
fn excluded_paths_and_always_excluded_directories_are_never_walked() {
    let dir = tempfile::tempdir().unwrap();
    for path in [
        "crates/core/src/lib.rs",
        "crates/fixtures/src/lib.rs",
        "crates/fixtures/repos/go/main.go",
        "tools/vendor/dep/dep.go",
        "web/node_modules/dep/index.js",
        "src/generated/types.ts",
    ] {
        write(dir.path(), path, "");
    }
    let exclude = Exclude::new(vec![
        "crates/fixtures/**".to_owned(),
        "generated".to_owned(),
    ])
    .unwrap();
    assert_eq!(
        walk_excluding(
            dir.path(),
            &LanguageRegistry::builtin(),
            &ProjectInfo::default(),
            &exclude
        ),
        ["crates/core/src/lib.rs"]
    );
}
