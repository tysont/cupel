// ABOUTME: The JaCoCo coverage producer for Maven projects: the configured command (JaCoCo's agent and report goals
// ABOUTME: run by coordinate around `mvn test`), whose JaCoCo XML lands in `target/site/jacoco/jacoco.xml` (ADR-108).
use cupel_core::coverage::CoverageMap;

use super::jacoco::source_paths;
use super::tool::{CoverageSpec, CoverageTool};

const ID: &str = "tool.jacoco-maven";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["java"],
    tool: "mvn",
    node_bin: false,
    version: &["mvn", "--version"],
    default_report: Some("target/site/jacoco/jacoco.xml"),
    resolve: source_paths,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});
