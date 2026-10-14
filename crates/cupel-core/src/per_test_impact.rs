// ABOUTME: What `tests --impacted` reports in the envelope's `tests` (ADR-077 §6): the impacted tests with their
// ABOUTME: reasons, a ready-to-run command per runner, and per language whether a per-test map chose them.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::id::{LangId, ProducerId};

/// The tests a scope impacts, and how to run them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestsReport {
    /// True only when every language in scope chose its tests from a per-test coverage map.
    pub per_test_map: bool,
    /// Sorted by language, then ID.
    pub impacted: Vec<ImpactedTest>,
    /// One per runner and project root with impacted tests.
    pub commands: Vec<TestCommand>,
    /// One per language in scope, in ID order.
    pub languages: Vec<ImpactBasis>,
}

/// One impacted test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ImpactedTest {
    /// The runner's test ID; a test file's path where the language fell back to file impact.
    pub id: String,
    /// The test's file, relative to the repository root, when known.
    pub file: Option<String>,
    /// Why the scope impacts it, such as `covers calc::add (src/lib.rs:5)`.
    pub reason: String,
    pub lang: LangId,
}

/// A command that runs one language's impacted tests in one project root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestCommand {
    pub lang: LangId,
    pub runner: ProducerId,
    /// The project root to run it in, relative to the repository root; `.` for the root.
    pub dir: String,
    /// Shell words, ready to paste in `dir`.
    pub command: String,
    /// False when the runner cannot narrow to the impacted tests and the command runs its whole
    /// suite.
    pub narrowed: bool,
}

/// How one language's impacted tests were chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ImpactBasis {
    pub lang: LangId,
    /// True when a per-test coverage map chose the tests; false when file-level impact through
    /// the import graph did.
    pub per_test_map: bool,
    /// The `PerTestMap` producer that measured the map, when one did.
    pub producer: Option<ProducerId>,
    /// Why the language fell back to file-level impact; null when it did not.
    pub fallback: Option<String>,
}
