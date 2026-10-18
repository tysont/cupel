// ABOUTME: Metamorphic relations (ADR-097): `[lang.<id>.relations.<name>]` as read and checked at load, the input-type
// ABOUTME: grammar generated inputs follow, and the `relation-results` fact the metamorphic engine produces.
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Table;
use crate::config::gate::is_segment;
use crate::fact::{Fact, FactEntry, FactKind};
use crate::finding::Location;
use crate::id::LangId;

/// The `[lang.<id>]` key holding the language's relations by name.
pub const RELATIONS_KEY: &str = "relations";

/// The keys a relation takes, in the order errors list them.
const KEYS: [&str; 5] = ["function", "input", "transform", "relation", "cases"];

/// One metamorphic relation: `relation(function(x), function(transform(x)))` holds for every
/// generated `x` of type `input`. `function`, `transform`, and `relation` are written in the
/// language of the table they are in; the engine's harness for that language reads them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    /// The function under test, as the language's harness names it (Python: `module:name`).
    pub function: String,
    /// What the generated inputs are.
    pub input: InputType,
    /// An expression for a callable from an input to its follow-up input.
    pub transform: String,
    /// An expression for a callable from the two outputs to whether the relation holds; `None`
    /// for equality.
    pub relation: Option<String>,
    /// How many inputs to generate; `None` for the engine's default.
    pub cases: Option<u64>,
}

/// The relations of `lang`'s `[lang.<id>]` table, by name; none when it has no `relations` key.
/// The problem otherwise, naming the table (`[lang.python.relations]`).
pub fn relations(lang: &LangId, table: &Table) -> Result<BTreeMap<String, Relation>, String> {
    let at = format!("[lang.{lang}.relations]");
    let Some(value) = table.get(RELATIONS_KEY) else {
        return Ok(BTreeMap::new());
    };
    let Value::Object(by_name) = value else {
        return Err(format!(
            "invalid `{at}`: expected a table of relations by name"
        ));
    };
    by_name
        .iter()
        .map(|(name, value)| {
            if !is_segment(name) {
                return Err(format!(
                    "invalid relation name `{name}` in `{at}`: must match `[a-z][a-z0-9_-]*`"
                ));
            }
            Relation::from_value(value)
                .map(|relation| (name.clone(), relation))
                .map_err(|reason| format!("invalid relation `{name}` in `{at}`: {reason}"))
        })
        .collect()
}

impl Relation {
    fn from_value(value: &Value) -> Result<Self, String> {
        let Value::Object(table) = value else {
            return Err("expected a table".to_owned());
        };
        if let Some(unknown) = table.keys().find(|key| !KEYS.contains(&key.as_str())) {
            let expected: Vec<String> = KEYS.iter().map(|key| format!("`{key}`")).collect();
            return Err(format!(
                "unknown key `{unknown}`; expected one of {}",
                expected.join(", ")
            ));
        }
        let input = required(table, "input")?;
        Ok(Self {
            function: required(table, "function")?,
            input: input.parse()?,
            transform: required(table, "transform")?,
            relation: optional(table, "relation")?,
            cases: cases(table)?,
        })
    }
}

fn required(table: &Table, key: &str) -> Result<String, String> {
    optional(table, key)?.ok_or_else(|| format!("needs `{key}`"))
}

fn optional(table: &Table, key: &str) -> Result<Option<String>, String> {
    match table.get(key) {
        None => Ok(None),
        Some(Value::String(text)) if !text.trim().is_empty() => Ok(Some(text.clone())),
        Some(_) => Err(format!("`{key}` must be a non-empty string")),
    }
}

fn cases(table: &Table) -> Result<Option<u64>, String> {
    match table.get("cases") {
        None => Ok(None),
        Some(value) => value
            .as_u64()
            .filter(|cases| *cases > 0)
            .map(Some)
            .ok_or_else(|| "`cases` must be a positive integer".to_owned()),
    }
}

/// What a generated input is: JSON values the engine generates and shrinks without knowing the
/// language, and the harness hands the function as the language's own values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputType {
    Bool,
    Int,
    Str,
    List(Box<InputType>),
    /// Spread into the function's positional arguments.
    Tuple(Vec<InputType>),
}

impl InputType {
    /// Whether the function takes the input's elements as separate arguments.
    pub fn spreads(&self) -> bool {
        matches!(self, Self::Tuple(_))
    }
}

impl fmt::Display for InputType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool => f.write_str("bool"),
            Self::Int => f.write_str("int"),
            Self::Str => f.write_str("str"),
            Self::List(element) => write!(f, "list[{element}]"),
            Self::Tuple(elements) => {
                let names: Vec<String> = elements.iter().map(ToString::to_string).collect();
                write!(f, "tuple[{}]", names.join(", "))
            }
        }
    }
}

impl FromStr for InputType {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let invalid = || {
            format!(
                "invalid input type `{}`: expected `bool`, `int`, `str`, `list[T]`, or \
                 `tuple[T, ...]`",
                text.trim()
            )
        };
        let mut parser = Parser { rest: text };
        let parsed = parser.input_type().ok_or_else(invalid)?;
        if parser.rest.trim().is_empty() {
            Ok(parsed)
        } else {
            Err(invalid())
        }
    }
}

/// A recursive-descent reader of the input-type grammar.
struct Parser<'a> {
    rest: &'a str,
}

impl Parser<'_> {
    fn input_type(&mut self) -> Option<InputType> {
        let name = self.word();
        match name {
            "bool" => Some(InputType::Bool),
            "int" => Some(InputType::Int),
            "str" => Some(InputType::Str),
            "list" => {
                self.expect('[')?;
                let element = self.input_type()?;
                self.expect(']')?;
                Some(InputType::List(Box::new(element)))
            }
            "tuple" => self.tuple(),
            _ => None,
        }
    }

    fn tuple(&mut self) -> Option<InputType> {
        self.expect('[')?;
        let mut elements = vec![self.input_type()?];
        while self.eat(',') {
            elements.push(self.input_type()?);
        }
        self.expect(']')?;
        Some(InputType::Tuple(elements))
    }

    fn word(&mut self) -> &str {
        self.rest = self.rest.trim_start();
        let end = self
            .rest
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(self.rest.len());
        let (word, rest) = self.rest.split_at(end);
        self.rest = rest;
        word
    }

    fn eat(&mut self, c: char) -> bool {
        self.rest = self.rest.trim_start();
        match self.rest.strip_prefix(c) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    fn expect(&mut self, c: char) -> Option<()> {
        self.eat(c).then_some(())
    }
}

/// The metamorphic engine's evaluation of every relation of one language (ADR-097).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelationResults {
    /// The run's seed; each relation's inputs derive from it and the relation's name.
    pub seed: u64,
    /// Every configured relation, by name.
    pub relations: Vec<RelationResult>,
}

impl Fact for RelationResults {
    const KIND: FactKind = FactKind::RelationResults;
    const VERSION: u32 = 1;
}

inventory::submit!(FactEntry::execution::<RelationResults>("relation-results"));

/// One relation evaluated on its generated inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelationResult {
    pub name: String,
    /// The function under test, as configured.
    pub function: String,
    /// The input type, printed canonically.
    pub input: String,
    /// How many generated inputs were evaluated: all of them, or up to the first violation.
    pub cases: u64,
    /// Where the function is defined, when the harness can tell.
    pub location: Option<Location>,
    /// The first generated input the relation did not hold for, minimized; `None` when it held
    /// for every one.
    pub violation: Option<Violation>,
}

/// A generated input the relation does not hold for, after minimization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Violation {
    /// The index of the generated case that first violated the relation.
    pub case: u64,
    /// The minimized input.
    pub input: Value,
    /// The input as generated, before minimization.
    pub original: Value,
    /// How many minimization steps led from `original` to `input`.
    pub shrinks: u32,
    /// The transform applied to `input`: the follow-up input.
    pub transformed: Evaluated,
    /// The function on `input`.
    pub source: Evaluated,
    /// The function on the follow-up input; `None` when the transform raised.
    pub followup: Option<Evaluated>,
    /// What the relation raised when it was called on the two outputs; `None` when it returned
    /// false, or when an output could not be computed.
    pub relation_raised: Option<String>,
}

/// How one evaluation in the harness ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Evaluated {
    /// It returned a value JSON can hold.
    Value(Value),
    /// It raised: the exception's type and message.
    Raised(String),
    /// It returned a value JSON cannot hold: the language's printed form of it.
    Repr(String),
}
