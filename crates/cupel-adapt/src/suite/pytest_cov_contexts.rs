// ABOUTME: The Python joint producer (ADR-084): one pytest run with pytest-cov's test contexts, whose JUnit report is the
// ABOUTME: baseline, whose LCOV report is the coverage, and whose contexts (`coverage json --show-contexts`) the per-test map.
use cupel_core::{FactKind, Result};

use super::{Suite, SuiteRun, SuiteSpec};
use crate::coverage::{parse_lcov, placed};
use crate::per_test::Site;
use crate::per_test::pytest_cov::coverage_of;
use crate::report::junit;
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
    per_test: Some("tool.pytest-cov-per-test"),
    programs: &["pytest", "coverage"],
    outputs: &["__pycache__"],
    fuses,
    run,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register_joint(Suite(&SPEC)),
});

/// The toolchain table's pytest-cov command, `<m> pytest --cov --cov-report=lcov:{out}`, with
/// any launcher that ends in `-m` (`python -m`, `uv run python -m`) or none.
fn fuses(words: &[&str]) -> bool {
    let Some(at) = words.iter().position(|word| *word == "pytest") else {
        return false;
    };
    let launcher = &words[..at];
    (launcher.is_empty() || launcher.last() == Some(&"-m"))
        && words[at..] == ["pytest", "--cov", "--cov-report=lcov:{out}"]
}

/// One pytest run with the runner's `JUnit` flags and pytest-cov's test contexts, its data file
/// under `.cupel/` so the repository's own coverage configuration still applies.
fn run(site: &Site<'_>) -> Result<SuiteRun> {
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
    let args: Vec<String> = [
        "-p",
        "no:cacheprovider",
        "-o",
        "junit_family=xunit1",
        "-o",
        "junit_logging=all",
        "--cov",
        "--cov-context=test",
    ]
    .map(str::to_owned)
    .into_iter()
    .chain([
        format!("--junitxml={}", junit.display()),
        format!("--cov-report=lcov:{}", lcov.display()),
    ])
    .collect();
    let captured = site.run("pytest", &args, &env)?;
    let found = std::fs::read_to_string(&junit)
        .ok()
        .and_then(|report| junit::pytest(&report));
    let command = std::iter::once("pytest".to_owned()).chain(args).collect();
    let invocation = finished(site.dir, command, &captured, found.as_deref(), &[0, 5]);
    let coverage = std::fs::read_to_string(&lcov)
        .ok()
        .and_then(|report| parse_lcov(&report).ok())
        .map(|map| placed(map, &[&site.path], "."));
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
                "pytest --cov=. --cov-branch --cov-report=lcov:{out}",
                "PYTHONDONTWRITEBYTECODE=1 pytest --cov --cov-report=lcov:{out}",
            ]
            .map(fused),
            [true, true, true, false, false]
        );
    }
}
