// ABOUTME: The `cupel.toml` file model (design §6.7): closed sections reject unknown keys, open tables
// ABOUTME: (`[lang.<id>]`, `[checks."<id>"]`) are syntax-checked here and registry-checked later (ADR-017).
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::ConfigError;
use crate::config::gate::{GateRuleSpec, is_segment, parse_check_id};
use crate::config::settings::{Format, Settings, TimeLimit};
use crate::error::Result;
use crate::grade::CostClass;
use crate::id::{CheckId, LangId};

/// An open table of values, as written in `cupel.toml`.
pub type Table = Map<String, Value>;

/// A parsed `cupel.toml`. Every section is optional.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    /// A semver requirement on the `cupel` binary, such as `>=0.4`.
    pub required_version: Option<String>,
    /// The branch `--changed` diffs against.
    pub default_branch: Option<String>,
    pub depth: Option<CostClass>,
    pub format: Option<Format>,
    pub budget: Option<TimeLimit>,
    /// Project roots of a monorepo, by name.
    #[serde(default)]
    pub roots: BTreeMap<String, RootConfig>,
    /// Tool and producer choices per language. Read by producers, never by checks.
    #[serde(default, deserialize_with = "lang_tables")]
    #[schemars(with = "BTreeMap<LangId, Table>")]
    pub lang: BTreeMap<LangId, Table>,
    /// Language-neutral check settings with per-language overrides. Read by checks only.
    #[serde(default, deserialize_with = "check_tables")]
    pub checks: BTreeMap<CheckId, CheckConfig>,
    #[serde(default)]
    pub oracles: BTreeMap<String, OracleConfig>,
    #[serde(default)]
    pub gate: GateConfig,
    /// Named settings selected with `--profile` or `CUPEL_PROFILE`.
    #[serde(default)]
    pub profile: BTreeMap<String, Settings>,
}

impl ConfigFile {
    /// Parses the text of the `cupel.toml` at `path`. `path` only labels errors.
    pub fn parse(text: &str, path: &Path) -> Result<Self> {
        toml::from_str(text).map_err(|err| {
            ConfigError::Invalid {
                path: path.to_owned(),
                message: err.to_string().trim_end().to_owned(),
            }
            .into()
        })
    }

    /// The top-level settings, the `cupel.toml` configuration layer.
    pub fn settings(&self) -> Settings {
        Settings {
            depth: self.depth,
            format: self.format,
            budget: self.budget,
        }
    }
}

/// One project root of a monorepo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RootConfig {
    /// Relative to the repository root.
    pub path: String,
    #[serde(deserialize_with = "lang_id")]
    pub lang: LangId,
}

/// One check's configuration: language-neutral parameters plus per-language overrides.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CheckConfig {
    /// `[checks."<id>"]` without its `lang` key.
    pub params: Table,
    /// `[checks."<id>".lang.<lang>]`, merged over `params` for that language.
    pub lang: BTreeMap<LangId, Table>,
}

const LANG_OVERRIDES: &str = "`lang` in a check table holds per-language override tables, \
                              such as `[checks.\"<id>\".lang.<lang>]`";

impl<'de> Deserialize<'de> for CheckConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut params = Table::deserialize(deserializer)?;
        let lang = match params.remove("lang") {
            None => BTreeMap::new(),
            Some(Value::Object(overrides)) => overrides
                .into_iter()
                .map(|(lang, table)| match table {
                    Value::Object(table) => Ok((validate_lang_id(lang)?, table)),
                    _ => Err(LANG_OVERRIDES.to_owned()),
                })
                .collect::<Result<_, String>>()
                .map_err(de::Error::custom)?,
            Some(_) => return Err(de::Error::custom(LANG_OVERRIDES)),
        };
        Ok(Self { params, lang })
    }
}

impl Serialize for CheckConfig {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        for (key, value) in &self.params {
            map.serialize_entry(key, value)?;
        }
        if !self.lang.is_empty() {
            map.serialize_entry("lang", &self.lang)?;
        }
        map.end()
    }
}

impl JsonSchema for CheckConfig {
    fn schema_name() -> Cow<'static, str> {
        "CheckConfig".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "description": "A check's parameters. The `lang` key holds per-language override tables, merged over the parameters for that language.",
            "properties": {
                "lang": {
                    "type": "object",
                    "additionalProperties": {"type": "object"}
                }
            },
            "additionalProperties": true
        })
    }
}

/// An oracle for differential and trace checks (design §6.5 `oracle`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OracleConfig {
    pub kind: OracleKind,
    /// A git ref, for `kind = "ref"`.
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    /// A command with an `{input}` placeholder, for `kind = "command"`.
    pub cmd: Option<String>,
    /// A specification file, for `kind = "spec"`.
    pub spec: Option<String>,
    /// The specification language (`tla+`, `quint`, `p`), for `kind = "spec"`.
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OracleKind {
    Ref,
    Command,
    Spec,
}

/// `[gate.changed]` applies to `--changed` scopes, `[gate.tree]` to whole-tree scopes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GateConfig {
    pub changed: Option<GateSection>,
    pub tree: Option<GateSection>,
}

/// One gate section's rules, in the order written.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GateSection(Vec<GateRuleSpec>);

impl GateSection {
    pub fn rules(&self) -> &[GateRuleSpec] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for GateSection {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RulesVisitor;

        impl<'de> Visitor<'de> for RulesVisitor {
            type Value = GateSection;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(
                    "a table of gate rules, such as `\"tests.mutation.score\" = \">= 0.80\"`",
                )
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<GateSection, A::Error> {
                let mut rules = Vec::new();
                while let Some((key, condition)) = map.next_entry::<String, String>()? {
                    rules.push(GateRuleSpec::parse(&key, &condition).map_err(de::Error::custom)?);
                }
                Ok(GateSection(rules))
            }
        }

        deserializer.deserialize_map(RulesVisitor)
    }
}

impl Serialize for GateSection {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for rule in &self.0 {
            map.serialize_entry(&rule.key.to_string(), &rule.condition.to_string())?;
        }
        map.end()
    }
}

impl JsonSchema for GateSection {
    fn schema_name() -> Cow<'static, str> {
        "GateSection".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "description": "Gate rules: `<check-id>.<metric>` with `>= > <= < ==` and a number (optionally `%`), or `<check-id>` with `none` or `no-breaking`.",
            "propertyNames": {
                "pattern": "^[a-z][a-z0-9_-]*\\.[a-z][a-z0-9_-]*(\\.[a-z][a-z0-9_-]*)?$"
            },
            "additionalProperties": {
                "type": "string",
                "pattern": "^\\s*((>=|>|<=|<|==)\\s*[-+]?[0-9.eE+-]+%?|none|no-breaking)\\s*$"
            }
        })
    }
}

/// A language ID's syntax; whether the language exists is a registry question (ADR-011).
fn validate_lang_id(raw: String) -> Result<LangId, String> {
    if is_segment(&raw) {
        Ok(LangId::from(raw))
    } else {
        Err(format!(
            "invalid language ID `{raw}`: must match `[a-z][a-z0-9_-]*`"
        ))
    }
}

fn lang_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<LangId, D::Error> {
    validate_lang_id(String::deserialize(deserializer)?).map_err(de::Error::custom)
}

fn lang_tables<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<LangId, Table>, D::Error> {
    BTreeMap::<String, Table>::deserialize(deserializer)?
        .into_iter()
        .map(|(lang, table)| Ok((validate_lang_id(lang)?, table)))
        .collect::<Result<_, String>>()
        .map_err(de::Error::custom)
}

fn check_tables<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<CheckId, CheckConfig>, D::Error> {
    BTreeMap::<String, CheckConfig>::deserialize(deserializer)?
        .into_iter()
        .map(|(id, config)| {
            parse_check_id(&id)
                .map(|id| (id, config))
                .map_err(|reason| format!("invalid check ID `{id}`: {reason}"))
        })
        .collect::<Result<_, String>>()
        .map_err(de::Error::custom)
}
