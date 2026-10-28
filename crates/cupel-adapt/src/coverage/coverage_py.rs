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
};

/// coverage.py lists every file under a source (`--source=<dir>` or `--source <dir>`) and,
/// without one, only the files the suite loaded (ADR-152).
fn listing(command: &str) -> Listing {
    if command
        .split_whitespace()
        .any(|word| word == "--source" || word.starts_with("--source="))
    {
        Listing::All
    } else {
        Listing::Loaded
    }
}

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});
