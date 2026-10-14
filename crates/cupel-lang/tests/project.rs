// ABOUTME: Tests for the `ProjectInfo` producers: every detect fixture yields its roots through the
// ABOUTME: registered languages, with members, packages, and skip dirs; Java and Clojure roots are unregistered.
mod support;

use std::path::{Path, PathBuf};

use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{FactKind, LangId, ProducerRegistry};
use cupel_lang::LanguageRegistry;
use support::{produce, tree};

fn detect_fixture(case: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cupel-fixtures/repos/detect")
        .join(case)
}

/// Every registered language's roots under `root`, ordered by path, then language.
fn all_roots(root: &Path) -> Vec<ProjectRoot> {
    let mut roots: Vec<ProjectRoot> = LanguageRegistry::builtin()
        .ids()
        .iter()
        .flat_map(|lang| produce::<ProjectInfo>(lang.as_str(), root).roots)
        .collect();
    roots.sort_by(|a, b| (&a.path, &a.lang).cmp(&(&b.path, &b.lang)));
    roots
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

const CARGO_SKIP: &[&str] = &["target"];
const GO_SKIP: &[&str] = &["vendor"];
const NODE_SKIP: &[&str] = &["node_modules", "dist", "coverage"];
const PYTHON_SKIP: &[&str] = &[".venv", "venv", "__pycache__", ".tox"];

/// A root with no members or packages.
fn plain(path: &str, lang: &str, build_files: &[&str], skip: &[&str]) -> ProjectRoot {
    ProjectRoot {
        path: path.to_owned(),
        lang: LangId::from(lang),
        build_files: strings(build_files),
        members: Vec::new(),
        packages: Vec::new(),
        skip_dirs: strings(skip),
    }
}

#[test]
fn single_ecosystem_fixtures_yield_one_root_each() {
    let go = ProjectRoot {
        packages: strings(&["./..."]),
        ..plain(".", "go", &["go.mod"], GO_SKIP)
    };
    let cases = [
        (
            "cargo-single",
            plain(".", "rust", &["Cargo.toml"], CARGO_SKIP),
        ),
        ("go-module", go),
        (
            "npm-js",
            plain(".", "javascript", &["package.json"], NODE_SKIP),
        ),
        (
            "python-pyproject",
            plain(".", "python", &["pyproject.toml"], PYTHON_SKIP),
        ),
        (
            "python-requirements",
            plain(".", "python", &["requirements.txt"], PYTHON_SKIP),
        ),
    ];
    for (case, root) in cases {
        assert_eq!(all_roots(&detect_fixture(case)), [root], "{case}");
    }
}

#[test]
fn workspace_fixtures_list_their_members_and_members_manifests() {
    let cargo = ProjectRoot {
        members: strings(&["crates/engine"]),
        ..plain(
            ".",
            "rust",
            &["Cargo.toml", "crates/engine/Cargo.toml"],
            CARGO_SKIP,
        )
    };
    assert_eq!(all_roots(&detect_fixture("cargo-workspace")), [cargo]);
    let pnpm = ProjectRoot {
        members: strings(&["packages/app"]),
        ..plain(
            ".",
            "typescript",
            &["package.json", "packages/app/package.json"],
            NODE_SKIP,
        )
    };
    assert_eq!(all_roots(&detect_fixture("pnpm-ts-workspace")), [pnpm]);
}

#[test]
fn the_monorepo_yields_four_roots_with_the_right_languages() {
    let roots = all_roots(&detect_fixture("monorepo"));
    let shape: Vec<(&str, &str)> = roots
        .iter()
        .map(|root| (root.path.as_str(), root.lang.as_str()))
        .collect();
    assert_eq!(
        shape,
        [
            ("engine", "rust"),
            ("ml", "python"),
            ("svc", "go"),
            ("web", "typescript")
        ]
    );
}

#[test]
fn maven_gradle_and_deps_edn_roots_are_unregistered() {
    assert!(all_roots(&detect_fixture("maven")).is_empty());
    assert_eq!(
        cupel_lang::project::unregistered_roots(&detect_fixture("maven")),
        [plain(
            ".",
            "java",
            &["pom.xml"],
            &["target", "build", ".gradle"]
        )]
    );
    let dir = tree(&[("android/build.gradle.kts", ""), ("tools/deps.edn", "{}\n")]);
    let roots = cupel_lang::project::unregistered_roots(dir.path());
    let shape: Vec<(&str, &str)> = roots
        .iter()
        .map(|root| (root.path.as_str(), root.lang.as_str()))
        .collect();
    assert_eq!(shape, [("android", "java"), ("tools", "clojure")]);
}

#[test]
fn an_empty_repository_has_no_roots() {
    assert!(all_roots(&detect_fixture("empty")).is_empty());
}

#[test]
fn a_go_workspace_is_one_root_covering_the_modules_it_uses() {
    let dir = tree(&[
        (
            "go.work",
            "go 1.23\n\nuse (\n\t./api // one\n\t./worker\n)\n",
        ),
        ("api/go.mod", "module example.com/api\n"),
        ("worker/go.mod", "module example.com/worker\n"),
    ]);
    let roots = all_roots(dir.path());
    assert_eq!(roots.len(), 1, "{roots:?}");
    assert_eq!(
        roots[0].build_files,
        ["go.work", "api/go.mod", "worker/go.mod"]
    );
    assert_eq!(roots[0].members, ["api", "worker"]);
    assert_eq!(roots[0].packages, ["./api/...", "./worker/..."]);
}

#[test]
fn a_go_workspace_that_uses_its_own_directory_covers_it_as_dot_slash_dots() {
    let dir = tree(&[
        ("go.work", "go 1.23\n\nuse ./shapes\nuse .\n"),
        ("go.mod", "module example.com/ledger\n"),
        ("shapes/go.mod", "module example.com/shapes\n"),
    ]);
    assert_eq!(all_roots(dir.path())[0].packages, ["./shapes/...", "./..."]);
}

#[test]
fn two_ecosystems_in_one_directory_are_two_roots_and_generated_dirs_are_skipped() {
    let dir = tree(&[
        ("Cargo.toml", "[package]\nname = \"x\"\n"),
        ("package.json", "{}"),
        ("node_modules/left-pad/package.json", "{}"),
        ("target/debug/build/go.mod", "module x\n"),
        (".hidden/pyproject.toml", ""),
    ]);
    let roots = all_roots(dir.path());
    assert_eq!(
        roots,
        [
            plain(".", "javascript", &["package.json"], NODE_SKIP),
            plain(".", "rust", &["Cargo.toml"], CARGO_SKIP),
        ]
    );
}

#[test]
fn a_package_is_typescript_when_it_or_a_member_has_a_tsconfig() {
    let dir = tree(&[
        ("package.json", "{}"),
        ("apps/web/package.json", "{}"),
        ("apps/web/tsconfig.json", "{}"),
    ]);
    assert!(
        produce::<ProjectInfo>("javascript", dir.path())
            .roots
            .is_empty()
    );
    let typescript = produce::<ProjectInfo>("typescript", dir.path()).roots;
    assert_eq!(typescript.len(), 1);
    assert_eq!(typescript[0].members, ["apps/web"]);
}

#[test]
fn every_language_registers_exactly_one_project_info_producer() {
    let languages = LanguageRegistry::builtin();
    let mut producers = ProducerRegistry::default();
    languages.register_producers(&mut producers);
    for lang in languages.langs() {
        let ids: Vec<String> = producers
            .producers_for(FactKind::ProjectInfo, &lang)
            .map(|producer| producer.id().to_string())
            .collect();
        assert_eq!(ids, [format!("lang.{}", lang.id)]);
    }
}
