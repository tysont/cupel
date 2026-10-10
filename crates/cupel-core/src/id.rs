// ABOUTME: Identifier types: open string IDs for checks, producers, tools, and languages,
// ABOUTME: and the run ID, a ULID carried with a `run_` prefix.
use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Declares a serde-transparent string identifier whose set of values is open.
macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

string_id!(
    /// A check's stable namespaced ID, such as `tests.mutation`.
    CheckId
);
string_id!(
    /// A producer's ID, such as `lang.go`, `tool.pytest-cov`, or `engine.mutation`.
    ProducerId
);
string_id!(
    /// An external tool a producer drives, such as `cargo-llvm-cov`.
    ToolId
);
string_id!(
    /// A language's ID, such as `rust` or `typescript`. Languages are registered by
    /// `cupel-lang`; core never enumerates them (ADR-011).
    LangId
);
string_id!(
    /// A family of languages whose symbols are comparable, such as `ecma` for JavaScript and
    /// TypeScript (ADR-011).
    LangFamily
);

/// A run's ID: `run_` followed by a ULID, so IDs sort by start time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RunId(ulid::Ulid);

impl RunId {
    const PREFIX: &'static str = "run_";

    /// A fresh run ID for a run starting now.
    pub fn generate() -> Self {
        Self(ulid::Ulid::generate())
    }
}

/// A random seed for a run without `--seed`: the low 64 of the 80 random bits of a fresh ULID,
/// drawn from the OS-seeded generator run IDs already use, and never 0 (ADR-020 §7).
pub fn random_seed() -> u64 {
    let bits = ulid::Ulid::generate().random();
    // Truncation keeps the low 64 random bits.
    #[allow(clippy::cast_possible_truncation)]
    let seed = bits as u64;
    seed.max(1)
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.0)
    }
}

impl FromStr for RunId {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        value
            .strip_prefix(Self::PREFIX)
            .and_then(|ulid| ulid::Ulid::from_string(ulid).ok())
            .map(Self)
            .ok_or_else(|| Error::InvalidId {
                what: "RunId",
                value: value.to_owned(),
                expected: "`run_` followed by a 26-character ULID",
            })
    }
}

impl TryFrom<String> for RunId {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        value.parse()
    }
}

impl From<RunId> for String {
    fn from(value: RunId) -> Self {
        value.to_string()
    }
}

impl JsonSchema for RunId {
    fn schema_name() -> Cow<'static, str> {
        "RunId".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": "^run_[0-9A-HJKMNP-TV-Z]{26}$",
            "description": "A run ID: `run_` followed by a ULID."
        })
    }
}
