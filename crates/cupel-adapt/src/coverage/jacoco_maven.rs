// ABOUTME: The JaCoCo coverage producers for Maven projects: the configured command (JaCoCo's goals by coordinate around
// ABOUTME: `mvn test`, or the project's own agent and report goal), whose XML lands in `target/site/jacoco/jacoco.xml` (ADR-108, ADR-209).
use cupel_core::coverage::CoverageMap;

use super::jacoco::source_paths;
use super::tool::{CoverageSpec, CoverageTool, every_file};

const ID: &str = "tool.jacoco-maven";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["java"],
    tool: "mvn",
    node_bin: false,
    version: &["mvn", "--version"],
    default_report: Some("target/site/jacoco/jacoco.xml"),
    resolve: source_paths,
    listing: every_file,
    providers: &[],
    uses: None,
};

/// The producer of a build that runs `JaCoCo`'s agent itself (ADR-209): `mvn test` and the
/// project's own `jacoco:report`.
static PROJECT: CoverageSpec = CoverageSpec {
    id: "tool.jacoco-maven-project",
    ..SPEC
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});

inventory::submit!(crate::ToolProducer {
    id: "tool.jacoco-maven-project",
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&PROJECT)),
});
