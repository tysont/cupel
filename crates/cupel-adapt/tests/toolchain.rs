// ABOUTME: Tests for the toolchain table: every row loads and is consistent, and selection over the
// ABOUTME: detect fixtures and scratch trees yields ADR-021's default commands for each root.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use cupel_adapt::toolchain::{self, Commands, Root};
use cupel_core::{LangId, ProducerId};

fn fixture(case: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cupel-fixtures/repos/detect")
        .join(case)
}

/// The commands the table chooses for `lang` at `dir` with `members` (relative) and `packages`.
fn commands(lang: &str, dir: &Path, members: &[&str], packages: &[&str]) -> Commands {
    let row = toolchain::for_lang(&LangId::from(lang)).expect("a row");
    let root = Root::new(
        dir.to_path_buf(),
        members.iter().map(|member| dir.join(member)).collect(),
        packages.iter().map(|p| (*p).to_owned()).collect(),
    );
    row.select(&root).commands()
}

fn expected(test: Option<&str>, coverage: Option<&str>, bench: Option<&str>) -> Commands {
    Commands {
        test: test.map(str::to_owned),
        coverage: coverage.map(str::to_owned),
        bench: bench.map(str::to_owned),
    }
}

const CARGO_COVERAGE: Option<&str> = Some("cargo llvm-cov --lcov --output-path {out}");

/// Asserts the commands the table chooses for a detect fixture's root (ADR-021's defaults).
fn assert_fixture(case: &str, path: &str, lang: &str, members: &[&str], want: &Commands) {
    let dir = fixture(case).join(path);
    let packages: &[&str] = if lang == "go" { &["./..."] } else { &[] };
    assert_eq!(
        &commands(lang, &dir, members, packages),
        want,
        "{case} {path}"
    );
}

#[test]
fn cargo_fixtures_get_cargo_test_or_nextest_llvm_cov_and_criterion() {
    assert_fixture(
        "cargo-single",
        ".",
        "rust",
        &[],
        &expected(Some("cargo test --workspace"), CARGO_COVERAGE, None),
    );
    assert_fixture(
        "cargo-workspace",
        ".",
        "rust",
        &["crates/engine"],
        &expected(
            Some("cargo nextest run --workspace"),
            CARGO_COVERAGE,
            Some("cargo bench --bench {name} -- --save-baseline {id}"),
        ),
    );
}

#[test]
fn the_go_fixture_gets_go_test_over_its_packages() {
    assert_fixture(
        "go-module",
        ".",
        "go",
        &[],
        &expected(
            Some("go test -json ./..."),
            Some("go test -coverprofile={out} ./..."),
            Some("go test -run=^$ -bench=. ./..."),
        ),
    );
}

#[test]
fn node_fixtures_get_their_runner_through_their_package_manager() {
    assert_fixture(
        "npm-js",
        ".",
        "javascript",
        &[],
        &expected(
            Some("npx jest"),
            Some("npx jest --coverage --coverageReporters=lcov"),
            None,
        ),
    );
    assert_fixture(
        "pnpm-ts-workspace",
        ".",
        "typescript",
        &["packages/app"],
        &expected(
            Some("pnpm vitest run"),
            Some("pnpm vitest run --coverage --coverage.reporter=lcov"),
            Some("pnpm run bench"),
        ),
    );
}

#[test]
fn python_fixtures_get_pytest_or_unittest_through_their_launcher() {
    assert_fixture(
        "python-pyproject",
        ".",
        "python",
        &[],
        &expected(
            Some("uv run python -m pytest"),
            Some("uv run python -m pytest --cov --cov-report=lcov:{out}"),
            Some("uv run python -m pytest --benchmark-only --benchmark-json={out}"),
        ),
    );
    assert_fixture(
        "python-requirements",
        ".",
        "python",
        &[],
        &expected(
            Some("python -m unittest"),
            Some("python -m coverage run -m unittest && python -m coverage lcov -o {out}"),
            None,
        ),
    );
}

#[test]
fn each_monorepo_root_gets_its_own_ecosystems_commands() {
    assert_fixture(
        "monorepo",
        "engine",
        "rust",
        &[],
        &expected(Some("cargo test --workspace"), CARGO_COVERAGE, None),
    );
    assert_fixture(
        "monorepo",
        "ml",
        "python",
        &[],
        &expected(
            Some("poetry run python -m pytest"),
            Some("poetry run python -m pytest --cov --cov-report=lcov:{out}"),
            None,
        ),
    );
    assert_fixture(
        "monorepo",
        "web",
        "typescript",
        &[],
        &expected(
            Some("npx vitest run"),
            Some("npx vitest run --coverage --coverage.reporter=lcov"),
            None,
        ),
    );
}

/// A temporary directory holding `files`, each a (relative path, text) pair.
fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (relative, text) in files {
        let path = dir.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    dir
}

#[test]
fn go_commands_cover_the_packages_project_info_reports() {
    let dir = tree(&[]);
    let got = commands("go", dir.path(), &[], &["./api/...", "./worker/..."]);
    assert_eq!(
        got.test.as_deref(),
        Some("go test -json ./api/... ./worker/...")
    );
    assert_eq!(
        got.bench.as_deref(),
        Some("go test -run=^$ -bench=. ./api/... ./worker/...")
    );
}

#[test]
fn yarn_and_node_test_are_chosen_without_other_signals() {
    let dir = tree(&[("package.json", "{\"name\": \"plain\"}"), ("yarn.lock", "")]);
    let got = commands("javascript", dir.path(), &[], &[]);
    assert_eq!(
        got,
        expected(
            Some("node --test"),
            Some("yarn c8 --reporter=lcov node --test"),
            None
        )
    );
}

#[test]
fn a_member_dependency_selects_the_runner_and_package_manager_fields_select_the_launcher() {
    let dir = tree(&[
        ("package.json", "{\"packageManager\": \"pnpm@9.1.0\"}"),
        (
            "libs/a/package.json",
            "{\"devDependencies\": {\"mocha\": \"1\"}}",
        ),
    ]);
    let got = commands("javascript", dir.path(), &["libs/a"], &[]);
    assert_eq!(
        got,
        expected(
            Some("pnpm mocha"),
            Some("pnpm c8 --reporter=lcov mocha"),
            None
        )
    );
}

#[test]
fn a_python_bench_needs_pytest_and_pytest_benchmark_alone_does_not_mean_pytest() {
    let dir = tree(&[("requirements-dev.txt", "pytest-benchmark==4\n")]);
    let got = commands("python", dir.path(), &[], &[]);
    assert_eq!(got.test.as_deref(), Some("python -m unittest"));
    assert_eq!(got.bench, None);
}

#[test]
fn selection_names_the_producers_it_chose() {
    let row = toolchain::for_lang(&LangId::from("rust")).unwrap();
    let root = Root::new(fixture("cargo-workspace"), Vec::new(), Vec::new());
    let selection = row.select(&root);
    let id = |tool: Option<&toolchain::Tool>| tool.map(|tool| tool.id.to_string());
    assert_eq!(id(selection.runner).as_deref(), Some("tool.cargo-nextest"));
    assert_eq!(
        id(selection.coverage).as_deref(),
        Some("tool.cargo-llvm-cov")
    );
    assert_eq!(
        id(selection.bench),
        None,
        "criterion is named only by the member"
    );
}

#[test]
fn every_launch_language_has_one_row_and_unknown_languages_none() {
    let found = ["javascript", "typescript", "rust", "go", "python", "java"]
        .map(|lang| toolchain::for_lang(&LangId::from(lang)).is_some());
    assert_eq!(found, [true, true, true, true, true, false]);
    let mut langs: Vec<&str> = toolchain::rows()
        .iter()
        .flat_map(|row| row.langs.iter().map(LangId::as_str))
        .collect();
    langs.sort_unstable();
    assert_eq!(langs, ["go", "javascript", "python", "rust", "typescript"]);
}

/// How `row` breaks the table's rules: every producer is `tool.<name>`, unique in its row, and
/// has a command; every runner a command map names is a runner of the row; every
/// `<placeholder>` is `<packages>` or a value every launcher sets; the last runner is the
/// fallback, detecting nothing.
fn inconsistencies(row: &toolchain::Row) -> Vec<String> {
    let runners: Vec<&ProducerId> = row.runners.iter().map(|tool| &tool.id).collect();
    let mut problems = Vec::new();
    let mut ids = BTreeSet::new();
    for tool in row.runners.iter().chain(&row.coverage).chain(&row.bench) {
        if !tool.id.as_str().starts_with("tool.") {
            problems.push(format!("{} is not tool.<name>", tool.id));
        }
        if !ids.insert(tool.id.clone()) {
            problems.push(format!("{} is listed twice", tool.id));
        }
        if tool.command.is_none() && tool.commands.is_empty() {
            problems.push(format!("{} has no command", tool.id));
        }
        for runner in tool
            .commands
            .keys()
            .filter(|runner| !runners.contains(runner))
        {
            problems.push(format!("{}: unknown runner {runner}", tool.id));
        }
        for template in tool.command.iter().chain(tool.commands.values()) {
            for name in placeholders(template) {
                if name != "packages" && !set_by_every_launcher(row, &name) {
                    problems.push(format!(
                        "{}: <{name}> is not set by every launcher",
                        tool.id
                    ));
                }
            }
        }
    }
    if !row
        .runners
        .last()
        .is_some_and(|tool| tool.detect.is_empty())
    {
        problems.push(format!(
            "the last runner of {:?} is not the fallback",
            row.langs
        ));
    }
    problems
}

/// Whether `row` has launchers and each of them sets the placeholder `name`.
fn set_by_every_launcher(row: &toolchain::Row, name: &str) -> bool {
    !row.launchers.is_empty()
        && row
            .launchers
            .iter()
            .all(|launcher| launcher.values.contains_key(name))
}

#[test]
fn every_row_is_consistent() {
    let problems: Vec<String> = toolchain::rows().iter().flat_map(inconsistencies).collect();
    assert_eq!(problems, Vec::<String>::new());
}

fn placeholders(template: &str) -> Vec<String> {
    template
        .split('<')
        .skip(1)
        .filter_map(|rest| rest.split_once('>').map(|(name, _)| name.to_owned()))
        .collect()
}

#[test]
fn programs_are_probed_with_version_unless_the_table_says_otherwise() {
    assert_eq!(toolchain::probe("cargo"), ["cargo", "--version"]);
    assert_eq!(toolchain::probe("go"), ["go", "version"]);
}

#[test]
fn a_row_describes_its_tools_rules_and_commands_in_default_order() {
    let row = toolchain::for_lang(&LangId::from("rust")).unwrap();
    let description = row.describe();
    let runners: Vec<(String, Vec<String>)> = description
        .runners
        .iter()
        .map(|tool| (tool.producer.to_string(), tool.detect.clone()))
        .collect();
    assert_eq!(
        runners,
        [
            (
                "tool.cargo-nextest".to_owned(),
                vec!["file `.config/nextest.toml` exists".to_owned()]
            ),
            ("tool.cargo-test".to_owned(), vec![]),
        ]
    );
    assert_eq!(description.coverage[0].format.as_deref(), Some("lcov"));
    assert_eq!(
        description.bench[0].detect,
        ["`Cargo.toml` in the root or a member mentions `criterion`"]
    );
    assert_eq!(
        description
            .race
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["miri", "loom"]
    );
}
