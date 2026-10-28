// ABOUTME: The Cloverage coverage producer for Clojure: the configured command (Cloverage run through the Clojure CLI
// ABOUTME: with `--lcov`), whose LCOV report, paths relative to the project, lands in `target/coverage/lcov.info` (ADR-109).
use cupel_core::coverage::CoverageMap;

use super::tool::{CoverageSpec, CoverageTool, as_written, every_file};

const ID: &str = "tool.cloverage";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["clojure"],
    tool: "clojure",
    node_bin: false,
    version: &["clojure", "--version"],
    default_report: Some("target/coverage/lcov.info"),
    resolve: as_written,
    listing: every_file,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});
