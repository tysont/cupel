// ABOUTME: Remediation hints a finding's action carries under fixed keys of `action.hints`: the subtree a clone
// ABOUTME: cluster shares, the deepest block of a complex function, and the cheapest import to cut (ADR-133).
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::finding::Location;
use crate::syntax::DecisionKind;

/// A hint with a fixed key in `action.hints`.
pub trait Hint: Serialize + DeserializeOwned + JsonSchema {
    /// The key it is stored under.
    const KEY: &'static str;
}

/// Stores `hint` in `hints` under its key.
pub fn insert<H: Hint>(hints: &mut Map<String, Value>, hint: &H) {
    if let Ok(value) = serde_json::to_value(hint) {
        hints.insert(H::KEY.to_owned(), value);
    }
}

/// The hint of type `H` in `hints`, when there is one of that shape.
pub fn read<H: Hint>(hints: &Map<String, Value>) -> Option<H> {
    hints
        .get(H::KEY)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

/// The largest normalized subtree every member of a clone cluster shares: the extraction
/// candidate, on `clone.cluster`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CommonSubtree {
    /// Nodes of the shared normalized subtree.
    pub nodes: u32,
    /// Its share of the first member's normalized nodes, to three decimals.
    pub share: f64,
    /// Where it is in the finding's symbol, the cluster's first member.
    pub location: Location,
    /// Where it is in each other member, in the evidence's member order.
    pub others: Vec<PlacedSubtree>,
    /// The identifiers and literals whose text differs between members: what the extracted
    /// function takes as parameters.
    pub parameters: Vec<Parameter>,
}

impl Hint for CommonSubtree {
    const KEY: &'static str = "common_subtree";
}

/// The shared subtree in one member of a clone cluster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlacedSubtree {
    /// The member's display name.
    pub symbol: String,
    pub location: Location,
}

/// One position of the shared subtree whose text differs between members.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Parameter {
    pub kind: ParameterKind,
    /// The text in each member, the first member's first.
    pub values: Vec<String>,
    /// How many positions in the subtree have these values.
    pub occurrences: u32,
}

/// What a differing position is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ParameterKind {
    Identifier,
    Literal,
}

/// The function's most deeply nested block: the extraction candidate, on `complexity.high`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeepestBlock {
    pub location: Location,
    /// Enclosing blocks that nest it, 0 at the top of the body.
    pub nesting: u32,
    /// The construct kinds from the outermost enclosing block to this block.
    pub path: Vec<DecisionKind>,
    /// What the decisions inside the block add to the function's cyclomatic complexity.
    pub cyclomatic: u32,
    /// What they add to its cognitive complexity, nesting increments included.
    pub cognitive: u32,
}

impl Hint for DeepestBlock {
    const KEY: &'static str = "deepest_block";
}

/// The import to remove to break an import cycle, on `dep.cycle`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheapestCut {
    /// The importing module.
    pub from: String,
    /// The imported module.
    pub to: String,
    /// The line of the first import statement.
    pub line: u32,
    /// The import statements' specifiers, as written.
    pub specifiers: Vec<String>,
    /// The names the statements import, sorted: what to move.
    pub names: Vec<String>,
    /// Statements that import the module whole or name nothing.
    pub unnamed: u32,
    /// Whether removing every import from `from` to `to` leaves the cycle's modules with no
    /// cycle at all.
    pub breaks_component: bool,
}

impl Hint for CheapestCut {
    const KEY: &'static str = "cheapest_cut";
}

/// The JSON Schema of `action.hints`: an open map whose known keys are each optional.
pub(crate) fn hints_schema(generator: &mut SchemaGenerator) -> Schema {
    let mut properties = Map::new();
    properties.insert(
        "nearest_test_file".to_owned(),
        json!({
            "type": "string",
            "description": "The test file nearest to the finding's site.",
            "x-cupel-optional": true,
        }),
    );
    properties.insert(
        "covering_tests".to_owned(),
        json!({
            "type": "array",
            "items": {"type": "string"},
            "description": "Tests that cover the finding's site.",
            "x-cupel-optional": true,
        }),
    );
    for (key, schema) in [
        (
            CommonSubtree::KEY,
            generator.subschema_for::<CommonSubtree>(),
        ),
        (DeepestBlock::KEY, generator.subschema_for::<DeepestBlock>()),
        (CheapestCut::KEY, generator.subschema_for::<CheapestCut>()),
    ] {
        let mut schema = schema;
        schema.insert("x-cupel-optional".to_owned(), Value::Bool(true));
        properties.insert(key.to_owned(), schema.to_value());
    }
    json_schema!({
        "type": "object",
        "description": "Kind-specific pointers: test hints and remediation hints (ADR-133); other keys are allowed.",
        "properties": properties,
        "additionalProperties": true,
    })
}

/// The JSON Schema of `H`, as a finding kind declares the hints it carries.
pub fn schema<H: Hint>() -> Map<String, Value> {
    match schemars::schema_for!(H).to_value() {
        Value::Object(schema) => schema,
        _ => Map::new(),
    }
}
