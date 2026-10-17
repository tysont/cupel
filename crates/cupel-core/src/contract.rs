// ABOUTME: The `ContractViolations` fact payload (ADR-113): the preconditions, postconditions, and invariants a
// ABOUTME: contract library reported violated while a language's tests ran with contract checking on, each with its test.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactKind};
use crate::id::ProducerId;
use crate::root_run::RootRun;

/// What one contract library reported while a language's tests ran with every contract it
/// checks turned on and generated inputs at the run's case budget (ADR-113).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContractViolations {
    /// The producer that reads the library's reports, such as `tool.rust-contracts`.
    pub tool: ProducerId,
    /// The baseline's runner, which ran the tests, such as `tool.cargo-test`.
    pub runner: ProducerId,
    /// Whether a project root declares the library as a dependency; when none does, the tests
    /// were not run again and nothing was checked.
    pub declared: bool,
    /// Each project root the runner ran in, in root order.
    pub runs: Vec<RootRun>,
    /// The tests the contract run reported, skipped ones aside.
    pub tests: u64,
    /// Every violation whose contract is in scope or outside the repository, in test order.
    pub violations: Vec<ContractViolation>,
}

impl Fact for ContractViolations {
    const KIND: FactKind = FactKind::ContractViolations;
    const VERSION: u32 = 1;
}

/// One contract a test violated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContractViolation {
    pub kind: ContractKind,
    /// The function, method, or type the contract is declared on, as the library names it;
    /// `None` when it does not say.
    pub function: Option<String>,
    /// The library's message: the contract's description and condition, with the values it saw
    /// when the library prints them.
    pub message: String,
    /// Where the contract is declared, relative to the repository root; `None` for a contract
    /// outside the repository, such as a dependency's.
    pub place: Option<ContractPlace>,
    /// The runner's ID of the test that violated it.
    pub test: String,
}

/// What a contract constrains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ContractKind {
    /// What must hold when a function is called: the caller's obligation.
    Precondition,
    /// What must hold when a function returns: the function's obligation.
    Postcondition,
    /// What must hold of a value before and after every method call on it.
    Invariant,
}

/// Where a contract is declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContractPlace {
    /// Relative to the repository root.
    pub file: String,
    pub line: u32,
}
