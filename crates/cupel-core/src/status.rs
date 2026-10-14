// ABOUTME: How a run ended: the envelope's `status` and the process exit code (design §6.6), each with
// ABOUTME: its name and meaning; below the envelope so events and descriptions can name them.
use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Overall outcome. `partial` and `error` are distinct from `fail`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Pass,
    Fail,
    Partial,
    Error,
    /// A `--dry-run` plan; nothing ran.
    Plan,
}

/// Process exit codes (design §6.6). Serialized as the integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum ExitCode {
    /// Ran fully; gate passed or no gate.
    Pass = 0,
    /// Ran fully; gate failed.
    Fail = 1,
    /// Bad flags or config; the envelope names the fix.
    Usage = 2,
    /// Missing tool, oracle, or toolchain; `doctor` findings attached.
    Environment = 3,
    /// Baseline test suite fails or build broken; scoring refused.
    Unevaluable = 4,
    /// Timeout or budget hit; the envelope has valid partial results.
    Partial = 5,
    /// Cupel bug; the envelope includes a bug-report payload.
    Internal = 6,
}

impl ExitCode {
    pub const ALL: [ExitCode; 7] = [
        Self::Pass,
        Self::Fail,
        Self::Usage,
        Self::Environment,
        Self::Unevaluable,
        Self::Partial,
        Self::Internal,
    ];

    pub fn code(self) -> u8 {
        self as u8
    }

    /// What the code means, as in the design's exit-code table.
    pub fn meaning(self) -> &'static str {
        EXIT_CODES[usize::from(self.code())].1
    }

    pub fn name(self) -> &'static str {
        EXIT_CODES[usize::from(self.code())].0
    }
}

impl TryFrom<u8> for ExitCode {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|code| code.code() == value)
            .ok_or(Error::OutOfRange {
                what: "exit code",
                value: value.into(),
                expected: "0 to 6",
            })
    }
}

impl From<ExitCode> for u8 {
    fn from(value: ExitCode) -> Self {
        value.code()
    }
}

impl JsonSchema for ExitCode {
    fn schema_name() -> Cow<'static, str> {
        "ExitCode".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "integer",
            "enum": [0, 1, 2, 3, 4, 5, 6],
            "description": "0 pass · 1 fail · 2 usage · 3 environment · 4 unevaluable · 5 partial · 6 internal"
        })
    }
}

/// Each exit code's name and meaning, indexed by the code.
const EXIT_CODES: [(&str, &str); 7] = [
    ("pass", "Ran fully; gate passed or no gate."),
    ("fail", "Ran fully; gate failed."),
    ("usage", "Bad flags or config; the envelope names the fix."),
    (
        "environment",
        "Missing tool, oracle, or toolchain; doctor findings attached.",
    ),
    (
        "unevaluable",
        "Baseline test suite fails or build broken; scoring refused.",
    ),
    (
        "partial",
        "Timeout or budget hit; the envelope has valid partial results.",
    ),
    (
        "internal",
        "Cupel bug; the envelope includes a bug-report payload.",
    ),
];
