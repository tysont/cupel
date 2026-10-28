// ABOUTME: The jest coverage producer for JavaScript and TypeScript: the configured command (`jest --coverage
// ABOUTME: --coverageReporters=lcov`), whose report lands in `coverage/lcov.info` unless it names `{out}`.
use cupel_core::coverage::CoverageMap;

use super::tool::{CoverageSpec, CoverageTool, as_written, every_file};

const ID: &str = "tool.jest-coverage";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["javascript", "typescript"],
    tool: "jest",
    node_bin: true,
    version: &["jest", "--version"],
    default_report: Some("coverage/lcov.info"),
    resolve: as_written,
    listing: every_file,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});
