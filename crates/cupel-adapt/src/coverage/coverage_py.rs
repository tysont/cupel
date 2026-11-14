// ABOUTME: The coverage.py coverage producer for Python, paired with the unittest runner: the configured command
// ABOUTME: (`coverage run -m unittest && coverage lcov -o {out}`), LCOV or Cobertura.
use cupel_core::coverage::{CoverageMap, Listing};

use super::tool::{CoverageSpec, CoverageTool, as_written};

const ID: &str = "tool.coverage-py";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["python"],
    tool: "coverage",
    node_bin: false,
    version: &["coverage", "--version"],
    default_report: None,
    resolve: as_written,
    listing,
    providers: &[],
    uses: None,
};

/// coverage.py lists every file under a source (`--source=<dir>` or `--source <dir>`) and,
/// without one, only the files the suite loaded (ADR-152), unless the root has no packages: its
/// project's configuration chooses the files (ADR-208).
fn listing(command: &str, packages: &[String]) -> Listing {
    let source = command
        .split_whitespace()
        .any(|word| word == "--source" || word.starts_with("--source="));
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
                "coverage run --source=src -m unittest && coverage lcov -o {out}",
                "python -m coverage run --source . -m unittest && python -m coverage lcov -o {out}",
                "coverage run -m unittest && coverage lcov -o {out}",
                "coverage run --sources-are-elsewhere -m unittest",
            ]
            .map(|command| listing(command, &[".".to_owned()])),
            [Listing::All, Listing::All, Listing::Loaded, Listing::Loaded]
        );
    }

    /// A root without packages is one whose project's configuration chooses the files
    /// (ADR-208): its report lists them, whatever the command.
    #[test]
    fn a_root_whose_project_chooses_its_files_lists_every_file() {
        assert_eq!(
            listing("coverage run -m unittest && coverage lcov -o {out}", &[]),
            Listing::All
        );
    }
}
