// ABOUTME: Tests for the `ProjectInfo` producers: every detect fixture yields its roots through the
// ABOUTME: registered languages, with members, packages, and skip dirs; no recognized root is unregistered.
mod support;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{Exclude, FactKind, LangId, ProducerRegistry};
use cupel_lang::LanguageRegistry;
use cupel_lang::project::Ecosystem;
use support::{produce, produce_excluding, tree};

fn detect_fixture(case: &str) -> PathBuf {
    support::fixture("detect").join(case)
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
const JAVA_SKIP: &[&str] = &["target", "build", ".gradle"];

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
fn maven_and_gradle_roots_are_java_roots_with_their_subprojects_as_members() {
    assert_eq!(
        all_roots(&detect_fixture("maven")),
        [plain(".", "java", &["pom.xml"], JAVA_SKIP)]
    );
    let dir = tree(&[
        ("android/build.gradle.kts", ""),
        ("android/settings.gradle.kts", "include(\"app\")\n"),
        ("android/app/build.gradle.kts", ""),
    ]);
    assert_eq!(
        produce::<ProjectInfo>("java", dir.path()).roots,
        [ProjectRoot {
            members: strings(&["app"]),
            ..plain(
                "android",
                "java",
                &[
                    "build.gradle.kts",
                    "settings.gradle.kts",
                    "app/build.gradle.kts"
                ],
                JAVA_SKIP
            )
        }]
    );
}

#[test]
fn deps_edn_roots_are_clojure_roots_and_no_recognized_root_is_unregistered() {
    let dir = tree(&[
        ("tools/deps.edn", "{:deps {lib/x {:local/root \"lib\"}}}\n"),
        ("tools/lib/deps.edn", "{}\n"),
        ("svc/pom.xml", "<project/>\n"),
    ]);
    assert_eq!(
        produce::<ProjectInfo>("clojure", dir.path()).roots,
        [ProjectRoot {
            members: strings(&["lib"]),
            ..plain(
                "tools",
                "clojure",
                &["deps.edn", "lib/deps.edn"],
                &["target", ".cpcache"]
            )
        }]
    );
    assert!(cupel_lang::unregistered_roots(dir.path(), &Exclude::default()).is_empty());
}

/// Every registered language's ecosystem, from the registry alone.
fn registered_ecosystems(registry: &LanguageRegistry) -> Vec<&'static Ecosystem> {
    registry
        .iter()
        .filter_map(|language| language.ecosystem())
        .collect()
}

#[test]
fn every_walk_skips_the_generated_directories_of_every_registered_ecosystem() {
    let ecosystems = registered_ecosystems(&LanguageRegistry::builtin());
    // Hidden directories are skipped anyway, so only the visible ones test the registry.
    let generated: BTreeSet<&str> = ecosystems
        .iter()
        .flat_map(|ecosystem| ecosystem.skip_dirs.iter().copied())
        .filter(|dir| !dir.starts_with('.'))
        .collect();
    let manifests: BTreeSet<&str> = ecosystems
        .iter()
        .map(|ecosystem| ecosystem.manifests[0])
        .collect();
    let files: Vec<String> = std::iter::once("control")
        .chain(generated.iter().copied())
        .flat_map(|parent| {
            manifests
                .iter()
                .enumerate()
                .map(move |(index, manifest)| format!("{parent}/p{index}/{manifest}"))
        })
        .collect();
    let dir = tree(
        &files
            .iter()
            .map(|path| (path.as_str(), "{}\n"))
            .collect::<Vec<_>>(),
    );
    let paths: BTreeSet<String> = all_roots(dir.path())
        .into_iter()
        .map(|root| root.path)
        .collect();
    let expected: BTreeSet<String> = (0..manifests.len())
        .map(|index| format!("control/p{index}"))
        .collect();
    assert_eq!(paths, expected);
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
    let ids: Vec<Vec<String>> = languages
        .langs()
        .iter()
        .map(|lang| {
            producers
                .producers_for(FactKind::ProjectInfo, lang)
                .map(|producer| producer.id().to_string())
                .collect()
        })
        .collect();
    let expected: Vec<Vec<String>> = languages
        .ids()
        .iter()
        .map(|id| vec![format!("lang.{id}")])
        .collect();
    assert_eq!(ids, expected);
}

#[test]
fn root_detection_does_not_descend_into_excluded_paths() {
    let dir = tree(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        ("crates/core/Cargo.toml", "[package]\nname = \"core\"\n"),
        ("crates/fixtures/repos/go/go.mod", "module x\n"),
        ("crates/fixtures/repos/py/pyproject.toml", ""),
        ("tools/vendor/dep/package.json", "{}"),
    ]);
    let exclude = Exclude::new(vec!["crates/fixtures/**".to_owned()]).unwrap();
    let roots: Vec<ProjectRoot> = LanguageRegistry::builtin()
        .ids()
        .iter()
        .flat_map(|lang| {
            produce_excluding::<ProjectInfo>(lang.as_str(), dir.path(), &exclude).roots
        })
        .collect();
    assert_eq!(
        roots,
        [ProjectRoot {
            members: strings(&["crates/core"]),
            ..plain(
                ".",
                "rust",
                &["Cargo.toml", "crates/core/Cargo.toml"],
                CARGO_SKIP
            )
        }]
    );
}
