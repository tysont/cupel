// ABOUTME: The c8 coverage producer for JavaScript and TypeScript, paired with mocha and node:test: the configured
// ABOUTME: command (`c8 --reporter=lcov node --test`), whose report lands in `coverage/lcov.info`.
use cupel_core::coverage::CoverageMap;

use super::tool::{CoverageSpec, CoverageTool, as_written};

const ID: &str = "tool.c8";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["javascript", "typescript"],
    tool: "c8",
    node_bin: true,
    version: &["c8", "--version"],
    default_report: Some("coverage/lcov.info"),
    resolve: as_written,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});
