// ABOUTME: The typed form of a language's `tables.toml`: the data the generic producers read
// ABOUTME: alongside the queries. Unknown keys are errors, so a typo fails the spec test.
use serde::Deserialize;

/// Everything a language's `tables.toml` may contain. Every section is optional; a section
/// a language omits takes the defaults documented on its type.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tables {}

impl Tables {
    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }
}
