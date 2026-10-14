// ABOUTME: The fact vocabulary: the closed set of fact kinds and the `Fact` trait payloads implement.
// ABOUTME: Facts are the only channel between producers (languages, tools, engines) and checks.
use std::any::Any;
use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::base::Range;
use crate::check::Manifest;
use crate::error::{Error, Result};
use crate::producer::SharedFact;

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
    CoverageMap,
    PerTestMap,
    MutationOutcomes,
    BenchSamples,
    PropertyResults,
    FuzzResults,
    RaceReports,
    AnalyzerReport,
    DivergenceResults,
}

impl fmt::Display for FactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FactKind {
    /// The kind's name as it appears in JSON, config, and the CLI.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Symbols => "symbols",
            Self::Decisions => "decisions",
            Self::NormTree => "norm-tree",
            Self::MutationSites => "mutation-sites",
            Self::TestFrames => "test-frames",
            Self::ModuleGraph => "module-graph",
            Self::ApiSurface => "api-surface",
            Self::Roots => "roots",
            Self::TestConventions => "test-conventions",
            Self::ProjectInfo => "project-info",
            Self::TestResults => "test-results",
            Self::CoverageMap => "coverage-map",
            Self::PerTestMap => "per-test-map",
            Self::MutationOutcomes => "mutation-outcomes",
            Self::BenchSamples => "bench-samples",
            Self::PropertyResults => "property-results",
            Self::FuzzResults => "fuzz-results",
            Self::RaceReports => "race-reports",
            Self::AnalyzerReport => "analyzer-report",
            Self::DivergenceResults => "divergence-results",
        }
    }

    /// Every kind, in catalog order.
    pub const ALL: [FactKind; 20] = [
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
        Self::CoverageMap,
        Self::PerTestMap,
        Self::MutationOutcomes,
        Self::BenchSamples,
        Self::PropertyResults,
        Self::FuzzResults,
        Self::RaceReports,
        Self::AnalyzerReport,
        Self::DivergenceResults,
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

/// A typed, versioned, language-neutral intermediate between producers and checks.
pub trait Fact: Serialize + DeserializeOwned + JsonSchema + Send + Sync + 'static {
    const KIND: FactKind;
    /// Bumped on an incompatible shape change; invalidates downstream caches.
    const VERSION: u32;
}

/// The facts a check may read: exactly the kinds its manifest declares in `needs` and `wants`.
/// Facts are shared, so one production serves every check that declares the kind. Under a
/// range scope, a check that declares `Check::base_needs` also reads those kinds as they were
/// at the base commit, and the range itself (ADR-056).
pub struct Facts {
    declared: BTreeSet<FactKind>,
    values: HashMap<FactKind, Arc<dyn Any + Send + Sync>>,
    base: HashMap<FactKind, Arc<dyn Any + Send + Sync>>,
    range: Option<Range>,
}

impl Facts {
    pub fn for_manifest(manifest: &Manifest) -> Self {
        Self {
            declared: manifest
                .needs
                .iter()
                .chain(&manifest.wants)
                .copied()
                .collect(),
            values: HashMap::new(),
            base: HashMap::new(),
            range: None,
        }
    }

    /// Adds a produced fact. Refuses kinds the manifest does not declare.
    pub fn insert<F: Fact>(&mut self, fact: F) -> Result<()> {
        self.check_declared(F::KIND)?;
        self.values.insert(F::KIND, Arc::new(fact));
        Ok(())
    }

    /// Adds a fact the scheduler produced once for several checks. Refuses kinds the manifest
    /// does not declare.
    pub fn insert_shared(&mut self, fact: &SharedFact) -> Result<()> {
        self.check_declared(fact.kind())?;
        self.values.insert(fact.kind(), fact.value());
        Ok(())
    }

    /// Adds a fact produced from the base side of the range. Refuses kinds the manifest does
    /// not declare.
    pub fn insert_base_shared(&mut self, fact: &SharedFact) -> Result<()> {
        self.check_declared(fact.kind())?;
        self.base.insert(fact.kind(), fact.value());
        Ok(())
    }

    /// Records the range the base side comes from.
    pub fn set_range(&mut self, range: Range) {
        self.range = Some(range);
    }

    /// The kinds the manifest declares, in catalog order.
    pub fn declared(&self) -> impl Iterator<Item = FactKind> + '_ {
        self.declared.iter().copied()
    }

    /// The fact of type `F`. Errors when `F` is undeclared or was not produced.
    pub fn get<F: Fact>(&self) -> Result<&F> {
        self.check_declared(F::KIND)?;
        self.values
            .get(&F::KIND)
            .and_then(|value| value.downcast_ref::<F>())
            .ok_or(Error::AbsentFact { kind: F::KIND })
    }

    /// The fact of type `F` as it was at the range's base commit, over the changed files that
    /// existed there. Errors when `F` is undeclared or no base side was produced.
    pub fn base<F: Fact>(&self) -> Result<&F> {
        self.check_declared(F::KIND)?;
        self.base
            .get(&F::KIND)
            .and_then(|value| value.downcast_ref::<F>())
            .ok_or(Error::AbsentFact { kind: F::KIND })
    }

    /// The range being compared; `None` outside a range scope.
    pub fn range(&self) -> Option<&Range> {
        self.range.as_ref()
    }

    fn check_declared(&self, kind: FactKind) -> Result<()> {
        if self.declared.contains(&kind) {
            Ok(())
        } else {
            Err(Error::UndeclaredFact { kind })
        }
    }
}
