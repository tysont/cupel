// ABOUTME: The Cloverage coverage producers for Leiningen projects (ADR-239): the configured command, Cloverage added by
// ABOUTME: coordinate through `lein update-in` or the project's own lein-cloverage task, whose LCOV lands in `target/coverage/lcov.info`.
use cupel_core::coverage::CoverageMap;

use super::tool::{CoverageSpec, CoverageTool, as_written, every_file};

const ID: &str = "tool.cloverage-lein";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["clojure"],
    tool: "lein",
    node_bin: false,
    version: &["lein", "version"],
    default_report: Some("target/coverage/lcov.info"),
    resolve: as_written,
    listing: every_file,
    providers: &[],
    uses: None,
    prepare: None,
    examples: None,
};

/// The producer of a project that declares the lein-cloverage plugin: its `lein cloverage` task.
static PLUGIN: CoverageSpec = CoverageSpec {
    id: "tool.lein-cloverage",
    ..SPEC
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});

inventory::submit!(crate::ToolProducer {
    id: "tool.lein-cloverage",
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&PLUGIN)),
});
