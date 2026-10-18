// ABOUTME: The `Footprint` fact payload (ADR-103): a language's dependencies from its lockfiles and manifests, their
// ABOUTME: licenses and known vulnerabilities when measured offline, configured artifact sizes, and the range's base side.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactEntry, FactKind};

/// What a language's projects depend on and how big their artifacts are, measured without
/// network access (ADR-103).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Footprint {
    /// The lockfiles and manifests read, relative to the repository root.
    pub manifests: Vec<String>,
    /// Every dependency they list, in the order read.
    pub dependencies: Vec<Dependency>,
    /// Whether licenses were read for these dependencies; a `None` license then means the
    /// dependency declares none.
    pub licenses: bool,
    /// Known vulnerabilities of the dependencies, from the configured advisories; `None` when
    /// no advisories are configured.
    pub vulnerabilities: Option<Vec<Vulnerability>>,
    /// The configured artifacts and their sizes; `None` when none are configured.
    pub artifacts: Option<Vec<Artifact>>,
    /// The dependencies at the range's base commit; `None` outside a range.
    pub base: Option<FootprintBase>,
}

impl Fact for Footprint {
    const KIND: FactKind = FactKind::Footprint;
    const VERSION: u32 = 1;
}

inventory::submit!(FactEntry::execution::<Footprint>("footprint"));

/// One dependency as a lockfile or manifest lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Dependency {
    pub name: String,
    /// The locked version; `None` when only a version range is declared.
    pub version: Option<String>,
    /// Whether the project declares it itself, rather than another dependency pulling it in.
    pub direct: bool,
    /// Its SPDX license expression or declared license, when the ecosystem records one.
    pub license: Option<String>,
    /// The lockfile or manifest that lists it, relative to the repository root.
    pub file: String,
    /// The line there that names it.
    pub line: u32,
}

impl Dependency {
    /// `name@version`, or `name` alone without a version.
    pub fn spelled(&self) -> String {
        match &self.version {
            Some(version) => format!("{}@{version}", self.name),
            None => self.name.clone(),
        }
    }
}

/// A known vulnerability of one dependency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Vulnerability {
    /// The advisory's ID, such as `RUSTSEC-2024-0001` or `GHSA-…`.
    pub id: String,
    /// The affected dependency's name and version.
    pub package: String,
    pub version: String,
    pub summary: String,
    /// The first version that fixes it, when the advisory names one.
    pub fixed: Option<String>,
}

/// A configured artifact and its size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Artifact {
    /// Relative to the repository root.
    pub path: String,
    pub bytes: u64,
}

/// The dependencies the same lockfiles and manifests listed at the range's base commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FootprintBase {
    pub commit: String,
    pub dependencies: Vec<Dependency>,
}
