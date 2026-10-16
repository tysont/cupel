// ABOUTME: The two scales every check and finding is graded on: evidence tier and cost class.
// ABOUTME: Tiers run 0 (structural metric) to 6 (proof); cost classes double as depth names.
use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Evidence grade of a check or finding, 0–6 (design §4). Higher is stronger evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Tier(u8);

impl Tier {
    pub const MAX: u8 = 6;

    pub fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Tier {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        if value <= Self::MAX {
            Ok(Self(value))
        } else {
            Err(Error::OutOfRange {
                what: "tier",
                value: value.into(),
                expected: "0 to 6",
            })
        }
    }
}

impl From<Tier> for u8 {
    fn from(value: Tier) -> Self {
        value.0
    }
}

impl JsonSchema for Tier {
    fn schema_name() -> Cow<'static, str> {
        "Tier".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "integer",
            "minimum": 0,
            "maximum": 6,
            "description": "Evidence tier: 0 structural metric … 6 proof."
        })
    }
}

/// How expensive a check or producer is to run. The same names select a run's depth (design §5.8).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum CostClass {
    Quick,
    Standard,
    Deep,
    Exhaustive,
}

impl std::fmt::Display for CostClass {
    /// The name a depth is selected by, such as `deep`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Quick => "quick",
            Self::Standard => "standard",
            Self::Deep => "deep",
            Self::Exhaustive => "exhaustive",
        })
    }
}

impl CostClass {
    /// The planning estimate for one target of work of this class, until the store has
    /// measured durations (ADR-041): 1 ms, 100 ms, 1 s, 10 s.
    pub fn est_ms_per_target(self) -> u64 {
        match self {
            Self::Quick => 1,
            Self::Standard => 100,
            Self::Deep => 1_000,
            Self::Exhaustive => 10_000,
        }
    }

    /// The planning estimate for one production by a producer of this class, until the store
    /// has recorded enough of its durations (ADR-086 §3): 100 ms, 30 s, 5 min, 50 min.
    pub fn est_ms_per_production(self) -> u64 {
        match self {
            Self::Quick => 100,
            Self::Standard => 30_000,
            Self::Deep => 300_000,
            Self::Exhaustive => 3_000_000,
        }
    }
}
