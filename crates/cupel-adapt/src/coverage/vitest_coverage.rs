// ABOUTME: The vitest coverage producer for JavaScript and TypeScript: the configured command (`vitest run
// ABOUTME: --coverage --coverage.reporter=lcov`), whose report lands in `coverage/lcov.info` unless it names `{out}`.
use cupel_core::coverage::CoverageMap;

use super::tool::{CoverageSpec, CoverageTool, as_written, every_file};

const ID: &str = "tool.vitest-coverage";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["javascript", "typescript"],
    tool: "vitest",
    node_bin: true,
    version: &["vitest", "--version"],
    default_report: Some("coverage/lcov.info"),
    resolve: as_written,
    listing: every_file,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});
