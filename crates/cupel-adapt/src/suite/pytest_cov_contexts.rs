// ABOUTME: The Python joint producer (ADR-084): one pytest run with pytest-cov's test contexts, whose JUnit report is the
// ABOUTME: baseline, whose LCOV report is the coverage, and whose contexts (`coverage json --show-contexts`) the per-test map.
use std::collections::BTreeSet;
use std::path::Path;

use cupel_core::{FactKind, LangId, Result};

use super::{COVERAGE, Suite, SuiteRun, SuiteSpec, probe};
use crate::coverage::{parse_lcov, placed, pytest_cov_listing};
use crate::per_test::Site;
use crate::per_test::pytest_cov::{PYTEST_COV, coverage_of};
use crate::report::{junit, listing};
use crate::runner::finished;

const ID: &str = "tool.pytest-cov-contexts";

static SPEC: SuiteSpec = SuiteSpec {
    id: ID,
    lang: "python",
    makes: &[
        (FactKind::TestResults, "tool.pytest"),
        (FactKind::CoverageMap, "tool.pytest-cov"),
        (FactKind::PerTestMap, "tool.pytest-cov-per-test"),
    ],
    per_test: Some(&PYTEST_COV),
    programs: &["pytest", "coverage"],
    outputs: &["__pycache__"],
    fuses,
    toolchain,
    run,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_joint(Suite(&SPEC)),
});

/// The toolchain table's pytest-cov command, `<m> pytest --cov=<source> --cov-report=lcov:{out}`,
/// with any launcher that ends in `-m` (`python -m`, `uv run python -m`) or none, and `--cov`
/// bare (as written before ADR-152) or with a source, once or more.
fn fuses(words: &[&str]) -> bool {
    let Some(at) = words.iter().position(|word| *word == "pytest") else {
        return false;
    };
    let launcher = &words[..at];
    let sources = words[at + 1..]
        .iter()
        .take_while(|word| is_cov(word))
        .count();
    (launcher.is_empty() || launcher.last() == Some(&"-m"))
        && sources > 0
        && words[at + 1 + sources..] == ["--cov-report=lcov:{out}"]
}

fn is_cov(word: &str) -> bool {
    word == "--cov" || word.starts_with("--cov=")
}

/// The `--cov` words the run passes: the configured command's sources, else the root's
/// `packages` (its source directory, ADR-152), else the bare `--cov`.
fn cov_words(site: &Site<'_>) -> Vec<String> {
    let configured: Vec<String> = site
        .ctx
        .ctx
        .lang
        .get(&LangId::from(SPEC.lang))
        .and_then(|table| table.get(COVERAGE)?.as_str())
        .into_iter()
        .flat_map(str::split_whitespace)
        .filter(|word| {
            word.strip_prefix("--cov=")
                .is_some_and(|source| !source.is_empty())
        })
        .map(str::to_owned)
        .collect();
    if !configured.is_empty() {
        return configured;
    }
    if site.packages.is_empty() {
        return vec!["--cov".to_owned()];
    }
    site.packages
        .iter()
        .map(|source| format!("--cov={source}"))
        .collect()
}

/// The interpreter and pytest the root runs under (ADR-149 §4): `python3 --version` (or
/// `python`'s) and `pytest --version`.
fn toolchain(root: &Path) -> Option<String> {
    let python =
        probe(root, "python3", &["--version"]).or_else(|| probe(root, "python", &["--version"]))?;
    Some(format!(
        "{python} · {}",
        probe(root, "pytest", &["--version"])?
    ))
}

/// Arguments that keep pytest from writing into the tree.
const QUIET: [&str; 2] = ["-p", "no:cacheprovider"];

/// One pytest run with the runner's `JUnit` flags and pytest-cov's test contexts, its data file
/// under `.cupel/` so the repository's own coverage configuration still applies; over every
/// collected test but `kept` (ADR-149 §1), named by node ID, or all of them when none is kept
/// or none remains.
fn run(site: &Site<'_>, kept: &BTreeSet<String>) -> Result<SuiteRun> {
    let mut selected = Vec::new();
    if !kept.is_empty() {
        let mut list: Vec<String> = QUIET.map(str::to_owned).to_vec();
        list.extend(["--collect-only".to_owned(), "-q".to_owned()]);
        let listed = listing::pytest(&site.succeed("pytest", &list, &[])?.stdout);
        selected = listed.into_iter().filter(|id| !kept.contains(id)).collect();
    }
    let data = site.report(".coverage");
    let junit = site.report("junit.xml");
    let lcov = site.report("coverage.lcov");
    let contexts = site.report("coverage.json");
    for path in [&data, &junit, &lcov, &contexts] {
        let _ = std::fs::remove_file(path);
    }
    let data_file = data.to_string_lossy().into_owned();
    let env = [
        ("PYTHONDONTWRITEBYTECODE", "1"),
        ("COVERAGE_FILE", data_file.as_str()),
    ];
    let mut args: Vec<String> = QUIET
        .into_iter()
        .chain(["-o", "junit_family=xunit1", "-o", "junit_logging=all"])
        .map(str::to_owned)
        .chain(cov_words(site))
        .chain([
            "--cov-context=test".to_owned(),
            format!("--junitxml={}", junit.display()),
            format!("--cov-report=lcov:{}", lcov.display()),
        ])
        .collect();
    let listing = pytest_cov_listing(&args.join(" "));
    args.extend(selected);
    let captured = site.run("pytest", &args, &env)?;
    let found = std::fs::read_to_string(&junit)
        .ok()
        .and_then(|report| junit::pytest(&report));
    let command = std::iter::once("pytest".to_owned()).chain(args).collect();
    let invocation = finished(site.dir, command, &captured, found.as_deref(), &[0, 5]);
    let coverage = std::fs::read_to_string(&lcov)
        .ok()
        .and_then(|report| parse_lcov(&report).ok())
        .map(|map| placed(map, &[&site.path], ".").listed(listing));
    let per_test = match coverage {
        Some(_) => per_test(site, &contexts, &env)?,
        None => None,
    };
    Ok(SuiteRun {
        invocation,
        tests: found.unwrap_or_default(),
        coverage,
        per_test,
    })
}

/// Each test's lines from the run's data, through `coverage json --show-contexts` into
/// `contexts`; `None` when it wrote no report.
fn per_test(
    site: &Site<'_>,
    contexts: &std::path::Path,
    env: &[(&str, &str)],
) -> Result<Option<Vec<cupel_core::TestCoverage>>> {
    let json = [
        "json".to_owned(),
        "--show-contexts".to_owned(),
        "-o".to_owned(),
        contexts.to_string_lossy().into_owned(),
    ];
    site.run("coverage", &json, env)?;
    Ok(std::fs::read_to_string(contexts)
        .ok()
        .map(|text| coverage_of(site, &text)))
}

#[cfg(test)]
mod tests {
    use super::fuses;

    #[test]
    fn the_default_command_with_any_launcher_is_fused_and_any_other_is_not() {
        let fused = |command: &str| fuses(&command.split_whitespace().collect::<Vec<_>>());
        assert_eq!(
            [
                "pytest --cov --cov-report=lcov:{out}",
                "python -m pytest --cov --cov-report=lcov:{out}",
                "uv run python -m pytest --cov --cov-report=lcov:{out}",
                "pytest --cov=src --cov-report=lcov:{out}",
                "uv run python -m pytest --cov=. --cov=libs --cov-report=lcov:{out}",
                "pytest --cov=. --cov-branch --cov-report=lcov:{out}",
                "pytest --cov-report=lcov:{out}",
                "PYTHONDONTWRITEBYTECODE=1 pytest --cov --cov-report=lcov:{out}",
            ]
            .map(fused),
            [true, true, true, true, true, false, false, false]
        );
    }
}
