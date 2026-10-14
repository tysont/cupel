// ABOUTME: The `cargo llvm-cov` coverage producer for Rust: the configured command (`cargo llvm-cov --lcov
// ABOUTME: --output-path {out}`), whose LCOV names files by absolute path and measures no branches on stable.
use cupel_core::coverage::CoverageMap;

use super::tool::{CoverageSpec, CoverageTool, as_written};

const ID: &str = "tool.cargo-llvm-cov";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["rust"],
    tool: "cargo-llvm-cov",
    node_bin: false,
    version: &["cargo", "llvm-cov", "--version"],
    default_report: None,
    resolve: as_written,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});
