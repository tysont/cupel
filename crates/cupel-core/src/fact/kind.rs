// ABOUTME: The closed set of fact kinds (design §8.3.2), in catalog order. It is the one list a new kind edits;
// ABOUTME: everything else about a kind is its payload module's catalog entry (ADR-123).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One row of the fact catalog (design §8.3.2). Adding a kind requires an ADR.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum FactKind {
    Symbols,
    Decisions,
    NormTree,
    MutationSites,
    TestFrames,
    ModuleGraph,
    ApiSurface,
    Roots,
    TestConventions,
    ProjectInfo,
    TestResults,
    TestReruns,
    CoverageMap,
    PerTestMap,
    MutationOutcomes,
    BenchSamples,
    PropertyResults,
    FuzzResults,
    RaceReports,
    AnalyzerReport,
    DivergenceResults,
    RelationResults,
    Footprint,
    Histories,
    SimulationResults,
    Traces,
    ModelCheckResults,
    ProofResults,
    ContractViolations,
    References,
}
