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
    providers: &[],
    uses: None,
};

/// coverage.py lists every file under a source (`--cov=<source>`) and, without one (`--cov`, or
/// `--cov=` for no filtering), only the files the suite loaded (ADR-152), unless the root has no
/// packages: its project's configuration chooses the files, and a file it leaves out is not one
/// the suite failed to load (ADR-208).
pub(crate) fn listing(command: &str, packages: &[String]) -> Listing {
    let source = command.split_whitespace().any(|word| {
        word.strip_prefix("--cov=")
            .is_some_and(|source| !source.is_empty())
    });
    if source || packages.is_empty() {
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
            .map(|command| listing(command, &[".".to_owned()])),
            [
                Listing::All,
                Listing::All,
                Listing::Loaded,
                Listing::Loaded,
                Listing::Loaded
            ]
        );
    }

    /// A root without packages is one whose project's configuration chooses the files
    /// (ADR-208): its report lists them, whatever the command.
    #[test]
    fn a_root_whose_project_chooses_its_files_lists_every_file() {
        assert_eq!(
            listing("pytest --cov --cov-report=lcov:{out}", &[]),
            Listing::All
        );
    }
}
