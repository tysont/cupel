// ABOUTME: The JaCoCo coverage producer for Gradle builds that apply the `jacoco` plugin: the configured command
// ABOUTME: (`gradle test jacocoTestReport`), whose XML lands in `build/reports/jacoco/test/jacocoTestReport.xml` (ADR-108).
use cupel_core::coverage::CoverageMap;

use super::jacoco::source_paths;
use super::tool::{CoverageSpec, CoverageTool, every_file};

const ID: &str = "tool.jacoco-gradle";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["java"],
    tool: "gradle",
    node_bin: false,
    version: &["gradle", "--version"],
    default_report: Some("build/reports/jacoco/test/jacocoTestReport.xml"),
    resolve: source_paths,
    listing: every_file,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});
