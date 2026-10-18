// ABOUTME: The per-fact conformance harness (design §8.3.7): loads `conformance/<fact>.toml` and each
// ABOUTME: language's `repos/<lang>/inventory.toml`, runs the language's producers, checks every construct.
//! # Inventory format
//!
//! **`conformance/<fact>.toml`** lists the constructs every producer of one fact must handle:
//!
//! ```toml
//! fact = "decisions"                 # a syntactic FactKind, kebab-case
//!
//! [[constructs]]
//! id = "if"                          # kebab-case, unique within the file
//! summary = "A conditional branch."  # what code exhibits it, language-neutrally
//! within = "decisions"               # optional: the list field of the located entry to search
//! expect = { kind = "branch" }       # optional: some searched item must contain these values
//! relation = "same-hash"             # optional: "same-hash" | "different-hash" (needs `twin`)
//! ```
//!
//! **`repos/<lang>/inventory.toml`** maps every construct of every fact the language produces,
//! as `[<fact>.<construct>]` tables:
//!
//! ```toml
//! [decisions.if]
//! file = "src/decide.rs"             # path in the fixture repository
//! name = "decide::classify"          # what to locate in the fact (see below)
//! expect = { nesting = 0 }           # optional: merged over the construct's `expect`
//! twin = "decide::classify_renamed"  # only for constructs with a `relation`
//!
//! [decisions.ternary]
//! not-applicable = "Rust has no conditional operator; `if` is an expression."
//! ```
//!
//! A construct is **detected** when the harness locates the mapped entry in the produced fact,
//! takes the `within` list of each located entry (or the entries themselves), and finds an item
//! that contains every value in `expect` (objects compare key by key, recursively; arrays and
//! scalars compare exactly). With a `relation`, the entries located by `name` and `twin` must
//! have equal (`same-hash`) or unequal (`different-hash`) `hash` fields.
//!
//! How `file` and `name` locate entries, per fact:
//!
//! | Fact | Located entries |
//! |---|---|
//! | `symbols` | the symbol in `file` whose `display` is `name` |
//! | `decisions`, `norm-tree`, `mutation-sites` | the entry for that symbol, joined by `SymbolKey` |
//! | `roots` | every root of that symbol |
//! | `test-frames` | the case called `name` in `file`; without `name`, the file entry |
//! | `module-graph` | edges from `file` whose `specifier` is `name` |
//! | `api-surface` | items in `file` whose `path` is `name` |
//! | `test-conventions` | the pairing whose `source` is `file`, if its `tests` include `name` |
//! | `project-info` | roots whose `path` is `file` |
//! | `histories` | histories whose `name` is `file` |
//! | `simulation-results` | harnesses defined in `file` whose `name` is `name` |
//!
//! An execution fact in [`EXECUTION`] has an inventory too (ADR-119): its producers' real-tool
//! tests produce it over their own fixture repository (`repos/<check>/<lang>/`), whose
//! `inventory.toml` maps each construct to what the producer read there, and check it with
//! [`check_language`].
//!
//! A construct a language cannot exhibit is mapped to `not-applicable = "<reason>"`. Facts a
//! language has no producer for are not checked, so their mappings may be absent. An unmapped
//! construct, a mapping to an unknown fact or construct, and an undetected construct all fail.
//!
//! An inventory may also name an **execution fact** ([`EXECUTION`]), which tools make rather
//! than languages. Nothing maps it per language: each tool producer's tests make the fact from
//! the producer's conformance input and check it with [`unexhibited`], naming the constructs the
//! tool cannot exhibit with their reasons.
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::syntax::ProjectInfo;
use cupel_core::{
    Cancellation, Ctx, Event, FactKind, LangId, ProduceCtx, ProducerRegistry, Scope, Writer,
};
use cupel_lang::{LanguageRegistry, walk};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::repos::repo_path;

/// The constructs every producer of one fact must handle.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactInventory {
    pub fact: FactKind,
    pub constructs: Vec<Construct>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Construct {
    pub id: String,
    pub summary: String,
    #[serde(default)]
    pub within: Option<String>,
    #[serde(default)]
    pub expect: Option<Map<String, Value>>,
    #[serde(default)]
    pub relation: Option<Relation>,
}

/// How the two entries a `twin` mapping locates must compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Relation {
    SameHash,
    DifferentHash,
}

impl FactInventory {
    pub fn parse(text: &str) -> Result<Self, String> {
        let inventory: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        let mut seen = Vec::new();
        for construct in &inventory.constructs {
            if seen.contains(&&construct.id) {
                return Err(format!("construct `{}` is listed twice", construct.id));
            }
            seen.push(&construct.id);
        }
        Ok(inventory)
    }

    pub fn construct(&self, id: &str) -> Option<&Construct> {
        self.constructs.iter().find(|construct| construct.id == id)
    }
}

/// Every `conformance/<fact>.toml`, in file-name order.
///
/// # Panics
///
/// When an inventory cannot be read or parsed.
pub fn fact_inventories() -> Vec<FactInventory> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .map(|entries| entries.filter_map(|e| Some(e.ok()?.path())).collect())
        .unwrap_or_default();
    paths.retain(|path| path.extension().is_some_and(|ext| ext == "toml"));
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let text = fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            FactInventory::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        })
        .collect()
}

/// How one language exhibits a construct.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Mapping {
    pub file: Option<String>,
    pub name: Option<String>,
    pub twin: Option<String>,
    pub expect: Option<Map<String, Value>>,
    pub not_applicable: Option<String>,
}

/// One language's `inventory.toml`: fact name → construct ID → mapping.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageInventory {
    pub lang: LangId,
    pub facts: BTreeMap<String, BTreeMap<String, Mapping>>,
}

impl LanguageInventory {
    pub fn parse(lang: &LangId, text: &str) -> Result<Self, String> {
        let facts: BTreeMap<String, BTreeMap<String, Mapping>> =
            toml::from_str(text).map_err(|error| error.to_string())?;
        for (fact, constructs) in &facts {
            for (construct, mapping) in constructs {
                let place = format!("[{fact}.{construct}]");
                match (&mapping.not_applicable, &mapping.file) {
                    (Some(reason), _) if reason.trim().is_empty() => {
                        return Err(format!("{place}: not-applicable needs a reason"));
                    }
                    (Some(_), Some(_)) => {
                        return Err(format!("{place}: a mapping is a file or not-applicable"));
                    }
                    (None, None) => {
                        return Err(format!("{place}: needs `file` or `not-applicable`"));
                    }
                    _ => {}
                }
            }
        }
        Ok(Self {
            lang: lang.clone(),
            facts,
        })
    }

    /// `repos/<lang>/inventory.toml`.
    pub fn load(lang: &LangId) -> Result<Self, String> {
        let path = repo_path(lang.as_str()).join("inventory.toml");
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(lang, &text).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn mapping(&self, fact: FactKind, construct: &str) -> Option<&Mapping> {
        self.facts.get(fact.as_str())?.get(construct)
    }
}

/// Why a language does not conform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The language produces the fact but does not map this construct.
    Unmapped { fact: FactKind, construct: String },
    /// The mapped code was not detected by the producer.
    NotDetected {
        fact: FactKind,
        construct: String,
        reason: String,
    },
    /// The inventory names a fact or construct that does not exist.
    Unknown { what: String },
    /// Producing the fact failed, or the fixture did not parse cleanly.
    Production { fact: FactKind, message: String },
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unmapped { fact, construct } => {
                write!(f, "[{fact}.{construct}] is not mapped in inventory.toml")
            }
            Self::NotDetected {
                fact,
                construct,
                reason,
            } => write!(f, "[{fact}.{construct}] was not detected: {reason}"),
            Self::Unknown { what } => write!(f, "inventory.toml maps unknown {what}"),
            Self::Production { fact, message } => write!(f, "producing {fact}: {message}"),
        }
    }
}

/// Checks one language's inventory against the fact inventories. `produced` lists the facts
/// the language has producers for; only those are checked. `produce` yields a fact as JSON.
pub fn check_language(
    facts: &[FactInventory],
    lang: &LanguageInventory,
    produced: &[FactKind],
    mut produce: impl FnMut(FactKind) -> Result<Value, String>,
) -> Vec<Failure> {
    let mut failures = unknown_mappings(facts, lang);
    let mut cache: BTreeMap<FactKind, Result<Value, String>> = BTreeMap::new();
    let mut get = |kind: FactKind| cache.entry(kind).or_insert_with(|| produce(kind)).clone();
    for inventory in facts.iter().filter(|i| produced.contains(&i.fact)) {
        for construct in &inventory.constructs {
            let fact = inventory.fact;
            let failure = |reason: String| Failure::NotDetected {
                fact,
                construct: construct.id.clone(),
                reason,
            };
            let Some(mapping) = lang.mapping(fact, &construct.id) else {
                failures.push(Failure::Unmapped {
                    fact,
                    construct: construct.id.clone(),
                });
                continue;
            };
            if mapping.not_applicable.is_some() {
                continue;
            }
            let value = match get(fact) {
                Ok(value) => value,
                Err(message) => {
                    failures.push(Failure::Production { fact, message });
                    continue;
                }
            };
            let symbols = if keyed_by_symbol(fact) {
                get(FactKind::Symbols).unwrap_or(Value::Null)
            } else {
                Value::Null
            };
            if let Err(reason) = detect(fact, construct, mapping, &value, &symbols) {
                failures.push(failure(reason));
            }
        }
    }
    failures
}

fn unknown_mappings(facts: &[FactInventory], lang: &LanguageInventory) -> Vec<Failure> {
    let mut failures = Vec::new();
    for (fact, constructs) in &lang.facts {
        let Some(inventory) = facts.iter().find(|i| i.fact.as_str() == fact) else {
            failures.push(Failure::Unknown {
                what: format!("fact `{fact}`"),
            });
            continue;
        };
        for construct in constructs.keys() {
            if inventory.construct(construct).is_none() {
                failures.push(Failure::Unknown {
                    what: format!("construct `{fact}.{construct}`"),
                });
            }
        }
    }
    failures
}

fn keyed_by_symbol(fact: FactKind) -> bool {
    matches!(
        fact,
        FactKind::Decisions | FactKind::NormTree | FactKind::MutationSites | FactKind::Roots
    )
}

/// Whether the construct mapped by `mapping` is present in `value`, the produced fact.
fn detect(
    fact: FactKind,
    construct: &Construct,
    mapping: &Mapping,
    value: &Value,
    symbols: &Value,
) -> Result<(), String> {
    let file = mapping.file.as_deref().unwrap_or_default();
    let entries = locate(fact, value, symbols, file, mapping.name.as_deref())?;
    if let Some(relation) = construct.relation {
        let twin = mapping
            .twin
            .as_deref()
            .ok_or("the construct compares two entries; the mapping needs `twin`")?;
        let twins = locate(fact, value, symbols, file, Some(twin))?;
        let same = entries[0].get("hash") == twins[0].get("hash");
        return match (relation, same) {
            (Relation::SameHash, true) | (Relation::DifferentHash, false) => Ok(()),
            (Relation::SameHash, false) => Err(format!("`{twin}` hashes differently")),
            (Relation::DifferentHash, true) => Err(format!("`{twin}` hashes the same")),
        };
    }
    let mut expect = construct.expect.clone().unwrap_or_default();
    expect.extend(mapping.expect.clone().unwrap_or_default());
    let expect = Value::Object(expect);
    let items: Vec<&Value> = match &construct.within {
        Some(field) => entries
            .iter()
            .filter_map(|entry| entry.get(field).and_then(Value::as_array))
            .flatten()
            .collect(),
        None => entries.iter().collect(),
    };
    if items.iter().any(|item| contains(item, &expect)) {
        Ok(())
    } else {
        Err(format!(
            "no {} matches {expect} among {}",
            construct.within.as_deref().unwrap_or("entry"),
            Value::Array(items.into_iter().cloned().collect())
        ))
    }
}

/// The entries of `value` that `file` and `name` locate (see the module docs); never empty.
fn locate(
    fact: FactKind,
    value: &Value,
    symbols: &Value,
    file: &str,
    name: Option<&str>,
) -> Result<Vec<Value>, String> {
    let list = |value: &Value, field: &str| -> Vec<Value> {
        value
            .get(field)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let is = |entry: &Value, field: &str, wanted: &str| entry.get(field) == Some(&wanted.into());
    let name_is = |entry: &Value, field: &str| name.is_none_or(|name| is(entry, field, name));
    let found: Vec<Value> = match fact {
        FactKind::Symbols => list(value, "symbols")
            .into_iter()
            .filter(|s| is(s, "file", file) && name_is(s, "display"))
            .collect(),
        FactKind::Decisions | FactKind::NormTree | FactKind::MutationSites | FactKind::Roots => {
            let symbol = locate(FactKind::Symbols, symbols, &Value::Null, file, name)?;
            let key = symbol_key(&symbol[0]);
            let field = if fact == FactKind::Roots {
                "roots"
            } else {
                "symbols"
            };
            list(value, field)
                .into_iter()
                .filter(|entry| entry.get("symbol") == Some(&key))
                .collect()
        }
        FactKind::TestFrames => {
            let files: Vec<Value> = list(value, "files")
                .into_iter()
                .filter(|f| is(f, "file", file))
                .collect();
            match name {
                None => files,
                Some(_) => files
                    .iter()
                    .flat_map(|f| list(f, "cases"))
                    .filter(|case| name_is(case, "name"))
                    .collect(),
            }
        }
        FactKind::ModuleGraph => list(value, "roots")
            .iter()
            .flat_map(|root| list(root, "edges"))
            .filter(|edge| is(edge, "from", file) && name_is(edge, "specifier"))
            .collect(),
        FactKind::ApiSurface => list(value, "items")
            .into_iter()
            .filter(|item| is(item, "file", file) && name_is(item, "path"))
            .collect(),
        FactKind::TestConventions => list(value, "pairs")
            .into_iter()
            .filter(|pair| {
                is(pair, "source", file)
                    && name.is_none_or(|name| list(pair, "tests").contains(&name.into()))
            })
            .collect(),
        FactKind::ProjectInfo => list(value, "roots")
            .into_iter()
            .filter(|root| is(root, "path", file))
            .collect(),
        FactKind::Histories => list(value, "histories")
            .into_iter()
            .filter(|history| is(history, "name", file))
            .collect(),
        FactKind::SimulationResults => list(value, "harnesses")
            .into_iter()
            .filter(|harness| {
                harness.pointer("/location/file") == Some(&file.into()) && name_is(harness, "name")
            })
            .collect(),
        other => return Err(format!("`{other}` has no conformance inventory")),
    };
    if found.is_empty() {
        Err(format!(
            "nothing in {file} called {}",
            name.map_or_else(|| "anything".to_owned(), |name| format!("`{name}`"))
        ))
    } else {
        Ok(found)
    }
}

/// The `SymbolKey` of a symbol entry, as it serializes.
fn symbol_key(symbol: &Value) -> Value {
    serde_json::json!({
        "file": symbol.get("file"),
        "line": symbol.pointer("/span/line"),
        "col": symbol.pointer("/span/col"),
    })
}

/// Execution facts whose inventories their tool producers' tests check with [`unexhibited`]
/// (ADR-116): no language exhibits these constructs, the tool does, so each producer makes the
/// fact from its own conformance input (a fixture its real tool runs on, or its captured output).
pub const EXECUTION: [FactKind; 6] = [
    FactKind::Histories,
    FactKind::SimulationResults,
    FactKind::Traces,
    FactKind::ModelCheckResults,
    FactKind::ProofResults,
    FactKind::ContractViolations,
];

/// `conformance/<kind>.toml`.
///
/// # Panics
///
/// When the kind has no inventory.
pub fn fact_inventory(kind: FactKind) -> FactInventory {
    fact_inventories()
        .into_iter()
        .find(|inventory| inventory.fact == kind)
        .unwrap_or_else(|| panic!("conformance/{kind}.toml is missing"))
}

/// The constructs of an execution fact's `inventory` that `value`, a fact a producer made, does
/// not exhibit: no item of the construct's `within` list (or the fact itself) contains its
/// `expect`. A construct the producer cannot exhibit is listed in `not_applicable` with its
/// reason; one listed with an empty reason must be exhibited, and one the inventory does not
/// have is a failure.
pub fn unexhibited(
    inventory: &FactInventory,
    value: &Value,
    not_applicable: &[(&str, &str)],
) -> Vec<Failure> {
    let fact = inventory.fact;
    let mut failures: Vec<Failure> = not_applicable
        .iter()
        .filter(|(construct, _)| inventory.construct(construct).is_none())
        .map(|(construct, _)| Failure::Unknown {
            what: format!("construct `{fact}.{construct}`"),
        })
        .collect();
    let excused = |id: &str| {
        not_applicable
            .iter()
            .any(|(construct, reason)| *construct == id && !reason.trim().is_empty())
    };
    for construct in inventory.constructs.iter().filter(|c| !excused(&c.id)) {
        let expect = Value::Object(construct.expect.clone().unwrap_or_default());
        let items: Vec<&Value> = match &construct.within {
            Some(field) => value
                .get(field)
                .and_then(Value::as_array)
                .map(|items| items.iter().collect())
                .unwrap_or_default(),
            None => vec![value],
        };
        if !items.iter().any(|item| contains(item, &expect)) {
            failures.push(Failure::NotDetected {
                fact,
                construct: construct.id.clone(),
                reason: format!(
                    "no {} matches {expect}",
                    construct.within.as_deref().unwrap_or("fact")
                ),
            });
        }
    }
    failures
}

/// Whether `actual` contains `expected`: objects key by key, recursively; anything else exactly.
pub fn contains(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => expected
            .iter()
            .all(|(key, value)| actual.get(key).is_some_and(|a| contains(a, value))),
        _ => actual == expected,
    }
}

/// Produces every fact `lang` has a producer for over its fixture repository and checks its
/// inventory. A fixture file that does not parse cleanly is a failure too.
///
/// # Panics
///
/// When the language is not registered.
pub fn run_language(lang: &LangId) -> Vec<Failure> {
    let languages = LanguageRegistry::builtin();
    let language = languages
        .get(lang)
        .unwrap_or_else(|| panic!("{lang} is not a registered language"));
    let mut producers = ProducerRegistry::default();
    languages.register_producers(&mut producers);
    let registered = cupel_lang::lang(language.as_ref());
    let produced: Vec<FactKind> = FactKind::SYNTACTIC
        .into_iter()
        .filter(|kind| producers.producers_for(*kind, &registered).next().is_some())
        .collect();
    let inventory = match LanguageInventory::load(lang) {
        Ok(inventory) => inventory,
        Err(message) => return vec![Failure::Unknown { what: message }],
    };
    check_language(&fact_inventories(), &inventory, &produced, |kind| {
        produce_fixture_fact(&producers, &languages, &registered, kind)
    })
}

/// One fact for `lang` over its fixture repository, as JSON.
///
/// # Errors
///
/// When no producer is registered, production fails, or a fixture file does not parse.
pub fn produce_fixture_fact(
    producers: &ProducerRegistry,
    languages: &LanguageRegistry,
    lang: &cupel_core::Lang,
    kind: FactKind,
) -> Result<Value, String> {
    produce_fact_at(
        producers,
        languages,
        lang,
        kind,
        &repo_path(lang.id.as_str()),
    )
}

/// [`produce_fixture_fact`] over the repository at `root`, such as a copy of the fixture.
///
/// # Errors
///
/// As [`produce_fixture_fact`].
pub fn produce_fact_at(
    producers: &ProducerRegistry,
    languages: &LanguageRegistry,
    lang: &cupel_core::Lang,
    kind: FactKind,
    root: &std::path::Path,
) -> Result<Value, String> {
    let root = root.to_path_buf();
    let producer = producers
        .producers_for(kind, lang)
        .next()
        .ok_or_else(|| format!("no producer of {kind} for {}", lang.id))?;
    let writer = Writer::new(&root, false)
        .map_err(|e| e.to_string())?
        .confined();
    let ctx = ProduceCtx::new(
        Ctx::new(writer.root().to_path_buf()),
        writer,
        Cancellation::new(),
    );
    let scope = Scope::of_files(walk(&root, languages, &ProjectInfo::default()));
    let mut events = Vec::new();
    let fact = producer
        .produce(&ctx, &scope, &mut events)
        .map_err(|e| e.to_string())?;
    let problems: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            Event::Finding(finding) => Some(format!(
                "{}: {}",
                finding.title,
                finding.detail.clone().unwrap_or_default()
            )),
            _ => None,
        })
        .collect();
    if problems.is_empty() {
        Ok(fact.to_json())
    } else {
        Err(problems.join("; "))
    }
}
