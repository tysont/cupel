// ABOUTME: Node's own coverage producer for JavaScript and TypeScript, paired with node:test: the configured
// ABOUTME: command (`node --test --experimental-test-coverage --test-reporter=lcov ...`) writes LCOV with no package.
use cupel_core::coverage::{CoverageMap, Listing};

use super::tool::{CoverageSpec, CoverageTool, as_written};

const ID: &str = "tool.node-test-coverage";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["javascript", "typescript"],
    tool: "node",
    node_bin: false,
    version: &["node", "--version"],
    default_report: None,
    resolve: as_written,
    listing,
    providers: &[],
    uses: Some("--experimental-test-coverage"),
};

/// Node lists only the files the suite loaded, and leaves its test files out.
fn listing(_: &str, _: &[String]) -> Listing {
    Listing::Loaded
}

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});
