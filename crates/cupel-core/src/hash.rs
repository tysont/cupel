// ABOUTME: Structural hashes and the finding ID derived from them.
// ABOUTME: Finding IDs take only structural inputs, so they survive reformatting and relocation.
use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::id::CheckId;

/// Declares a string newtype holding lowercase hex after a fixed prefix.
macro_rules! hex_id {
    ($(#[$meta:meta])* $name:ident, prefix = $prefix:literal, digits = $digits:expr, pattern = $pattern:literal, expected = $expected:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
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

        impl FromStr for $name {
            type Err = Error;

            fn from_str(value: &str) -> Result<Self> {
                if is_prefixed_hex(value, $prefix, $digits) {
                    Ok(Self(value.to_owned()))
                } else {
                    Err(Error::InvalidId {
                        what: stringify!($name),
                        value: value.to_owned(),
                        expected: $expected,
                    })
                }
            }
        }

        impl TryFrom<String> for $name {
            type Error = Error;

            fn try_from(value: String) -> Result<Self> {
                value.parse()
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl JsonSchema for $name {
            fn schema_name() -> Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!({
                    "type": "string",
                    "pattern": $pattern,
                    "description": $expected
                })
            }
        }
    };
}

hex_id!(
    /// A symbol's identity: `sh_` plus the hex digest of its normalized syntax tree. Survives
    /// rename, move, and reformatting.
    SymbolHash,
    prefix = "sh_",
    digits = None,
    pattern = "^sh_[0-9a-f]+$",
    expected = "`sh_` followed by lowercase hex"
);
hex_id!(
    /// The structural position of a site (a mutation site, a clone span) within its symbol,
    /// as lowercase hex. Never derived from line numbers.
    SiteHash,
    prefix = "",
    digits = None,
    pattern = "^[0-9a-f]+$",
    expected = "lowercase hex"
);
hex_id!(
    /// A finding's stable ID: `fnd_` plus the first 24 bits of a BLAKE3 digest, as hex.
    FindingId,
    prefix = "fnd_",
    digits = Some(6),
    pattern = "^fnd_[0-9a-f]{6}$",
    expected = "`fnd_` followed by six lowercase hex digits"
);

impl FindingId {
    /// Derives a finding's ID from structural inputs only (design §7.2, ADR-013).
    ///
    /// `site` is `None` when the finding concerns the whole symbol. A line number cannot be
    /// passed, so IDs are stable across reformatting and relocation:
    ///
    /// ```compile_fail
    /// # use cupel_core::{CheckId, FindingId};
    /// let line: u32 = 142;
    /// FindingId::compute(&CheckId::from("tests.mutation"), "mutant.survived", &line, None);
    /// ```
    pub fn compute(
        check: &CheckId,
        kind: &str,
        symbol: &SymbolHash,
        site: Option<&SiteHash>,
    ) -> Self {
        const SEPARATOR: &[u8] = &[0];
        let mut hasher = blake3::Hasher::new();
        hasher.update(check.as_str().as_bytes());
        hasher.update(SEPARATOR);
        hasher.update(kind.as_bytes());
        hasher.update(SEPARATOR);
        hasher.update(symbol.as_str().as_bytes());
        hasher.update(SEPARATOR);
        hasher.update(site.map_or("", SiteHash::as_str).as_bytes());
        let digest = hasher.finalize();
        let [a, b, c, ..] = *digest.as_bytes();
        Self(format!("fnd_{a:02x}{b:02x}{c:02x}"))
    }
}

fn is_prefixed_hex(value: &str, prefix: &str, digits: Option<usize>) -> bool {
    let Some(hex) = value.strip_prefix(prefix) else {
        return false;
    };
    let well_formed = !hex.is_empty()
        && hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    well_formed && digits.is_none_or(|n| hex.len() == n)
}
