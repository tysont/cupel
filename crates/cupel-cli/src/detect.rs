// ABOUTME: Detects a repository's project roots, their languages, default commands, and directories
// ABOUTME: to skip, as the `ProjectInfo` payload `init` turns into `cupel.toml` (ADR-021).
//!
//! The walk and the root rules live here; each ecosystem module decides what one root is. Plan
//! step P1.9 moves each ecosystem's `describe` into its language's `ProjectInfo` producer.
mod cargo;
mod go;
mod jvm;
mod node;
mod python;

use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::LangId;
use cupel_core::syntax::{DefaultCommands, ProjectInfo, ProjectRoot};

/// One build ecosystem: the files that mark its roots, the directories it generates, and how a
/// root of it is described.
pub struct Ecosystem {
    pub manifests: &'static [&'static str],
    pub skip_dirs: &'static [&'static str],
    pub describe: fn(&Candidate) -> ProjectRoot,
}

/// Every ecosystem detection knows, in no particular order; roots are sorted afterwards.
const ECOSYSTEMS: [&Ecosystem; 6] = [
    &cargo::ECOSYSTEM,
    &go::ECOSYSTEM,
    &node::ECOSYSTEM,
    &python::ECOSYSTEM,
    &jvm::JAVA,
    &jvm::CLOJURE,
];

/// A directory holding an ecosystem's manifest, with the directories below it that hold that
/// ecosystem's manifests too (workspace members).
pub struct Candidate {
    ecosystem: &'static Ecosystem,
    /// Relative to the repository root, `/`-separated; `.` for the root itself.
    pub path: String,
    pub dir: PathBuf,
    pub members: Vec<PathBuf>,
}

impl Candidate {
    /// The root directory, then each member directory.
    pub fn dirs(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.dir.as_path()).chain(self.members.iter().map(PathBuf::as_path))
    }

    /// Whether `relative` exists in the root directory.
    pub fn has(&self, relative: &str) -> bool {
        self.dir.join(relative).exists()
    }

    /// The text of `relative` in the root directory, if it can be read.
    pub fn read(&self, relative: &str) -> Option<String> {
        fs::read_to_string(self.dir.join(relative)).ok()
    }

    /// Whether any file named `file` in the root or a member mentions the package `name`.
    pub fn mentions(&self, file: &str, name: &str) -> bool {
        self.dirs()
            .any(|dir| fs::read_to_string(dir.join(file)).is_ok_and(|text| mentions(&text, name)))
    }

    /// The `ProjectRoot` for this candidate: its manifests, then its members' manifests.
    pub fn root(&self, lang: &str, commands: DefaultCommands) -> ProjectRoot {
        let build_files = self
            .dirs()
            .flat_map(|dir| {
                self.ecosystem
                    .manifests
                    .iter()
                    .filter(move |manifest| dir.join(manifest).is_file())
                    .map(move |manifest| relative(&dir.join(manifest), &self.dir))
            })
            .collect();
        ProjectRoot {
            path: self.path.clone(),
            lang: LangId::from(lang),
            build_files,
            commands,
            skip_dirs: self
                .ecosystem
                .skip_dirs
                .iter()
                .map(|dir| (*dir).to_owned())
                .collect(),
        }
    }
}

/// Every project root under `root`, ordered by path, then language.
pub fn project(root: &Path) -> ProjectInfo {
    let mut candidates = Vec::new();
    walk(root, root, &mut candidates);
    let mut roots: Vec<ProjectRoot> = candidates
        .iter()
        .map(|candidate| (candidate.ecosystem.describe)(candidate))
        .collect();
    roots.sort_by(|a, b| (&a.path, &a.lang).cmp(&(&b.path, &b.lang)));
    ProjectInfo { roots }
}

/// Visits `dir` and the directories below it in sorted order. A directory with an ecosystem's
/// manifest becomes a root of it, or a member of the root above it when there is one.
fn walk(root: &Path, dir: &Path, candidates: &mut Vec<Candidate>) {
    for ecosystem in ECOSYSTEMS {
        if !ecosystem
            .manifests
            .iter()
            .any(|manifest| dir.join(manifest).is_file())
        {
            continue;
        }
        let enclosing = candidates.iter_mut().find(|candidate| {
            std::ptr::eq(candidate.ecosystem, ecosystem) && dir.starts_with(&candidate.dir)
        });
        match enclosing {
            Some(candidate) => candidate.members.push(dir.to_path_buf()),
            None => candidates.push(Candidate {
                ecosystem,
                path: relative(dir, root),
                dir: dir.to_path_buf(),
                members: Vec::new(),
            }),
        }
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut subdirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            !name.starts_with('.') && !skipped(&name)
        })
        .map(|entry| entry.path())
        .collect();
    subdirs.sort();
    for subdir in subdirs {
        walk(root, &subdir, candidates);
    }
}

/// Whether some ecosystem generates directories named `name`.
fn skipped(name: &str) -> bool {
    ECOSYSTEMS
        .iter()
        .any(|ecosystem| ecosystem.skip_dirs.contains(&name))
}

/// `path` relative to `base`, `/`-separated; `.` when they are the same.
fn relative(path: &Path, base: &Path) -> String {
    let parts: Vec<String> = path
        .strip_prefix(base)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

/// Whether `text` contains `name` as a whole package name: not preceded or followed by a
/// character that can be part of one, so `pytest-benchmark` does not mention `pytest`.
pub fn mentions(text: &str, name: &str) -> bool {
    let part_of_name = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    text.match_indices(name).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + name.len()..].chars().next();
        !before.is_some_and(part_of_name) && !after.is_some_and(part_of_name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One expected root: path, language, build files, test, coverage, bench, skip directories.
    type Expected<'a> = (
        &'a str,
        &'a str,
        &'a [&'a str],
        Option<&'a str>,
        Option<&'a str>,
        Option<&'a str>,
        &'a [&'a str],
    );

    const CARGO_COVERAGE: Option<&str> = Some("cargo llvm-cov --lcov --output-path {out}");
    const CARGO_SKIP: &[&str] = &["target"];
    const GO_SKIP: &[&str] = &["vendor"];
    const NODE_SKIP: &[&str] = &["node_modules", "dist", "coverage"];
    const PYTHON_SKIP: &[&str] = &[".venv", "venv", "__pycache__", ".tox"];

    fn fixture(case: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cupel-fixtures/repos/detect")
            .join(case)
    }

    fn expected(roots: &[Expected<'_>]) -> ProjectInfo {
        ProjectInfo {
            roots: roots
                .iter()
                .map(
                    |(path, lang, build_files, test, coverage, bench, skip)| ProjectRoot {
                        path: (*path).to_owned(),
                        lang: LangId::from(*lang),
                        build_files: build_files.iter().map(|f| (*f).to_owned()).collect(),
                        commands: DefaultCommands {
                            test: test.map(str::to_owned),
                            coverage: coverage.map(str::to_owned),
                            bench: bench.map(str::to_owned),
                        },
                        skip_dirs: skip.iter().map(|d| (*d).to_owned()).collect(),
                    },
                )
                .collect(),
        }
    }

    /// Every detect fixture and the roots it must yield.
    const CASES: &[(&str, &[Expected<'_>])] = &[
        (
            "cargo-single",
            &[(
                ".",
                "rust",
                &["Cargo.toml"],
                Some("cargo test --workspace"),
                CARGO_COVERAGE,
                None,
                CARGO_SKIP,
            )],
        ),
        (
            "cargo-workspace",
            &[(
                ".",
                "rust",
                &["Cargo.toml", "crates/engine/Cargo.toml"],
                Some("cargo nextest run --workspace"),
                CARGO_COVERAGE,
                Some("cargo bench --bench {name} -- --save-baseline {id}"),
                CARGO_SKIP,
            )],
        ),
        (
            "go-module",
            &[(
                ".",
                "go",
                &["go.mod"],
                Some("go test -json ./..."),
                Some("go test -coverprofile={out} ./..."),
                Some("go test -run=^$ -bench=. ./..."),
                GO_SKIP,
            )],
        ),
        (
            "npm-js",
            &[(
                ".",
                "javascript",
                &["package.json"],
                Some("npx jest"),
                Some("npx jest --coverage --coverageReporters=lcov"),
                None,
                NODE_SKIP,
            )],
        ),
        (
            "pnpm-ts-workspace",
            &[(
                ".",
                "typescript",
                &["package.json", "packages/app/package.json"],
                Some("pnpm vitest run"),
                Some("pnpm vitest run --coverage --coverage.reporter=lcov"),
                Some("pnpm run bench"),
                NODE_SKIP,
            )],
        ),
        (
            "python-pyproject",
            &[(
                ".",
                "python",
                &["pyproject.toml"],
                Some("uv run python -m pytest"),
                Some("uv run python -m pytest --cov --cov-report=lcov:{out}"),
                Some("uv run python -m pytest --benchmark-only --benchmark-json={out}"),
                PYTHON_SKIP,
            )],
        ),
        (
            "python-requirements",
            &[(
                ".",
                "python",
                &["requirements.txt"],
                Some("python -m unittest"),
                Some("python -m coverage run -m unittest && python -m coverage lcov -o {out}"),
                None,
                PYTHON_SKIP,
            )],
        ),
        (
            "monorepo",
            &[
                (
                    "engine",
                    "rust",
                    &["Cargo.toml"],
                    Some("cargo test --workspace"),
                    CARGO_COVERAGE,
                    None,
                    CARGO_SKIP,
                ),
                (
                    "ml",
                    "python",
                    &["pyproject.toml"],
                    Some("poetry run python -m pytest"),
                    Some("poetry run python -m pytest --cov --cov-report=lcov:{out}"),
                    None,
                    PYTHON_SKIP,
                ),
                (
                    "svc",
                    "go",
                    &["go.mod"],
                    Some("go test -json ./..."),
                    Some("go test -coverprofile={out} ./..."),
                    Some("go test -run=^$ -bench=. ./..."),
                    GO_SKIP,
                ),
                (
                    "web",
                    "typescript",
                    &["package.json"],
                    Some("npx vitest run"),
                    Some("npx vitest run --coverage --coverage.reporter=lcov"),
                    None,
                    NODE_SKIP,
                ),
            ],
        ),
        (
            "maven",
            &[(
                ".",
                "java",
                &["pom.xml"],
                None,
                None,
                None,
                &["target", "build", ".gradle"],
            )],
        ),
        ("empty", &[]),
    ];

    #[test]
    fn each_detect_fixture_yields_its_roots_languages_and_commands() {
        for (case, roots) in CASES {
            assert_eq!(project(&fixture(case)), expected(roots), "{case}");
        }
    }

    fn write(root: &Path, relative: &str, text: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn paths_and_langs(info: &ProjectInfo) -> Vec<(String, String)> {
        info.roots
            .iter()
            .map(|root| (root.path.clone(), root.lang.as_str().to_owned()))
            .collect()
    }

    #[test]
    fn a_go_workspace_is_one_root_testing_the_modules_it_uses() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "go.work",
            "go 1.23\n\nuse (\n\t./api\n\t./worker\n)\n",
        );
        write(dir.path(), "api/go.mod", "module example.com/api\n");
        write(dir.path(), "worker/go.mod", "module example.com/worker\n");
        let info = project(dir.path());
        assert_eq!(info.roots.len(), 1, "{info:?}");
        let root = &info.roots[0];
        assert_eq!(root.build_files, ["go.work", "api/go.mod", "worker/go.mod"]);
        assert_eq!(
            root.commands.test.as_deref(),
            Some("go test -json ./api/... ./worker/...")
        );
    }

    #[test]
    fn gradle_and_deps_edn_are_roots_without_commands() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "android/build.gradle.kts", "");
        write(dir.path(), "tools/deps.edn", "{}\n");
        let info = project(dir.path());
        assert_eq!(
            paths_and_langs(&info),
            [
                ("android".to_owned(), "java".to_owned()),
                ("tools".to_owned(), "clojure".to_owned())
            ]
        );
        assert!(info.roots.iter().all(|root| root.commands
            == DefaultCommands {
                test: None,
                coverage: None,
                bench: None
            }));
    }

    #[test]
    fn two_ecosystems_in_one_directory_are_two_roots_and_generated_dirs_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Cargo.toml", "[package]\nname = \"x\"\n");
        write(
            dir.path(),
            "package.json",
            "{\"devDependencies\": {\"mocha\": \"1\"}}",
        );
        write(dir.path(), "node_modules/left-pad/package.json", "{}");
        write(dir.path(), "target/debug/build/go.mod", "module x\n");
        write(dir.path(), ".hidden/pyproject.toml", "");
        let info = project(dir.path());
        assert_eq!(
            paths_and_langs(&info),
            [
                (".".to_owned(), "javascript".to_owned()),
                (".".to_owned(), "rust".to_owned())
            ]
        );
        assert_eq!(info.roots[0].commands.test.as_deref(), Some("npx mocha"));
        assert_eq!(info.roots[0].build_files, ["package.json"]);
    }

    #[test]
    fn yarn_and_node_test_are_chosen_without_other_signals() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "package.json", "{\"name\": \"plain\"}");
        write(dir.path(), "yarn.lock", "");
        let commands = project(dir.path()).roots.remove(0).commands;
        assert_eq!(commands.test.as_deref(), Some("node --test"));
        assert_eq!(
            commands.coverage.as_deref(),
            Some("yarn c8 --reporter=lcov node --test")
        );
    }

    #[test]
    fn mentions_matches_whole_package_names_only() {
        assert!(mentions("dev = [\"pytest>=8\"]", "pytest"));
        assert!(mentions("[tool.pytest.ini_options]", "pytest"));
        assert!(!mentions("pytest-benchmark", "pytest"));
        assert!(!mentions("my_pytest", "pytest"));
        assert!(mentions("pytest-benchmark==4", "pytest-benchmark"));
        assert!(!mentions("", "pytest"));
    }
}
