// ABOUTME: The coverage.py coverage producer for Python, paired with the unittest runner: the configured command
// ABOUTME: (`coverage run -m unittest && coverage lcov -o {out}`), LCOV or Cobertura.
use cupel_core::coverage::CoverageMap;

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
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});
