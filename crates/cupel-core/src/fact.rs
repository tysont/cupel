// ABOUTME: The fact vocabulary: the closed set of fact kinds (design §8.3.2) and the `Fact` trait payloads
// ABOUTME: implement. A leaf of cupel-core, so errors, schemas, and payloads can name a kind.
use std::fmt;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
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
}

impl fmt::Display for FactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FactKind {
    /// The kind's name as it appears in JSON, config, and the CLI.
    pub fn as_str(self) -> &'static str {
        NAMES[self as usize]
    }

    /// The `[lang.<id>]` key that pins this kind's producer, such as `runner = "unittest"` for
    /// `TestResults` (ADR-076), `race = "tsan"` for `RaceReports` (ADR-105), or `mutator = "plugin.pit"`
    /// for `MutationOutcomes` (ADR-110); `None` for kinds whose producer cannot be chosen.
    pub fn pin_key(self) -> Option<&'static str> {
        match self {
            Self::TestResults => Some("runner"),
            Self::RaceReports => Some("race"),
            Self::MutationOutcomes => Some("mutator"),
            _ => None,
        }
    }

    /// Every kind, in catalog order.
    pub const ALL: [FactKind; 25] = [
        Self::Symbols,
        Self::Decisions,
        Self::NormTree,
        Self::MutationSites,
        Self::TestFrames,
        Self::ModuleGraph,
        Self::ApiSurface,
        Self::Roots,
        Self::TestConventions,
        Self::ProjectInfo,
        Self::TestResults,
        Self::TestReruns,
        Self::CoverageMap,
        Self::PerTestMap,
        Self::MutationOutcomes,
        Self::BenchSamples,
        Self::PropertyResults,
        Self::FuzzResults,
        Self::RaceReports,
        Self::AnalyzerReport,
        Self::DivergenceResults,
        Self::RelationResults,
        Self::Footprint,
        Self::Histories,
        Self::SimulationResults,
    ];

    /// Kinds produced by language producers from the syntax tree.
    pub const SYNTACTIC: [FactKind; 10] = [
        Self::Symbols,
        Self::Decisions,
        Self::NormTree,
        Self::MutationSites,
        Self::TestFrames,
        Self::ModuleGraph,
        Self::ApiSurface,
        Self::Roots,
        Self::TestConventions,
        Self::ProjectInfo,
    ];
}

/// Each kind's name, in declaration order (so `NAMES[kind as usize]` names `kind`).
const NAMES: [&str; 25] = [
    "symbols",
    "decisions",
    "norm-tree",
    "mutation-sites",
    "test-frames",
    "module-graph",
    "api-surface",
    "roots",
    "test-conventions",
    "project-info",
    "test-results",
    "test-reruns",
    "coverage-map",
    "per-test-map",
    "mutation-outcomes",
    "bench-samples",
    "property-results",
    "fuzz-results",
    "race-reports",
    "analyzer-report",
    "divergence-results",
    "relation-results",
    "footprint",
    "histories",
    "simulation-results",
];

/// A typed, versioned, language-neutral intermediate between producers and checks.
pub trait Fact: Serialize + DeserializeOwned + JsonSchema + Send + Sync + 'static {
    const KIND: FactKind;
    /// Bumped on an incompatible shape change; invalidates downstream caches.
    const VERSION: u32;
}
