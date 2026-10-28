// ABOUTME: The pytest-cov coverage producer for Python, paired with the pytest runner: the configured command
// ABOUTME: (`pytest --cov=<source> --cov-report=lcov:{out}`), LCOV or Cobertura, branches with `--cov-branch`.
use cupel_core::coverage::{CoverageMap, Listing};

use super::tool::{CoverageSpec, CoverageTool, as_written};

const ID: &str = "tool.pytest-cov";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["python"],
    tool: "pytest",
    node_bin: false,
    version: &["pytest", "--version"],
    default_report: None,
    resolve: as_written,
    listing,
};

/// coverage.py lists every file under a source (`--cov=<source>`) and, without one (`--cov`, or
/// `--cov=` for no filtering), only the files the suite loaded (ADR-152).
pub(crate) fn listing(command: &str) -> Listing {
    if command.split_whitespace().any(|word| {
        word.strip_prefix("--cov=")
            .is_some_and(|source| !source.is_empty())
    }) {
        Listing::All
    } else {
        Listing::Loaded
    }
}

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});

#[cfg(test)]
mod tests {
    use cupel_core::coverage::Listing;

    use super::listing;

    #[test]
    fn a_command_with_a_source_lists_every_file_and_one_without_the_loaded_ones() {
        assert_eq!(
            [
                "pytest --cov=src --cov-report=lcov:{out}",
                "uv run python -m pytest --cov=. --cov-branch --cov-report=lcov:{out}",
                "pytest --cov --cov-report=lcov:{out}",
                "pytest --cov= --cov-report=lcov:{out}",
                "pytest --cov-report=lcov:{out}",
            ]
            .map(listing),
            [
                Listing::All,
                Listing::All,
                Listing::Loaded,
                Listing::Loaded,
                Listing::Loaded
            ]
        );
    }
}
