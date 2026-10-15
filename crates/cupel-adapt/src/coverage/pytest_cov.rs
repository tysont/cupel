// ABOUTME: The pytest-cov coverage producer for Python, paired with the pytest runner: the configured command
// ABOUTME: (`pytest --cov --cov-report=lcov:{out}`), LCOV or Cobertura, branches with `--cov-branch`.
use cupel_core::coverage::CoverageMap;

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
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});
