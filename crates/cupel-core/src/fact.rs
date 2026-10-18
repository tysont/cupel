// ABOUTME: The fact vocabulary: the fact kinds, the `Fact` trait payloads implement, and the catalog each
// ABOUTME: payload module joins with its own `inventory::submit!` entry (design §8.3.2, ADR-123).
mod kind;

use std::fmt;
use std::sync::OnceLock;

use schemars::{JsonSchema, Schema};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

pub use kind::FactKind;

use crate::producer::Produced;

impl fmt::Display for FactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FactKind {
    /// The kind's name as it appears in JSON, config, and the CLI.
    pub fn as_str(self) -> &'static str {
        self.entry().name
    }

    /// The `[lang.<id>]` key that pins this kind's producer, such as `runner = "unittest"` for
    /// `TestResults` (ADR-076), `race = "tsan"` for `RaceReports` (ADR-105), or `mutator = "plugin.pit"`
    /// for `MutationOutcomes` (ADR-110); `None` for kinds whose producer cannot be chosen.
    pub fn pin_key(self) -> Option<&'static str> {
        self.entry().pin_key
    }

    /// Every kind, in catalog order.
    pub fn all() -> &'static [FactKind] {
        &catalog().kinds
    }

    /// Kinds produced by language producers from the syntax tree, in catalog order.
    pub fn syntactic() -> &'static [FactKind] {
        &catalog().syntactic
    }

    /// This kind's catalog entry.
    ///
    /// # Panics
    ///
    /// When the kind's payload module submits no entry, which `tests/fact_catalog.rs` rules out.
    pub fn entry(self) -> &'static FactEntry {
        self.find_entry()
            .unwrap_or_else(|| panic!("fact kind {self:?} has no catalog entry"))
    }

    /// This kind's catalog entry, if its payload module submits one.
    pub(crate) fn find_entry(self) -> Option<&'static FactEntry> {
        let entries = &catalog().entries;
        entries
            .binary_search_by_key(&self, |entry| entry.kind)
            .ok()
            .map(|at| entries[at])
    }
}

/// A typed, versioned, language-neutral intermediate between producers and checks.
pub trait Fact: Serialize + DeserializeOwned + JsonSchema + Send + Sync + 'static {
    const KIND: FactKind;
    /// Bumped on an incompatible shape change; invalidates downstream caches.
    const VERSION: u32;
}

/// One row of the fact catalog, submitted by the kind's payload module with
/// `inventory::submit!(FactEntry::execution::<Payload>("name"))` (ADR-123).
pub struct FactEntry {
    pub kind: FactKind,
    /// The kind's name as it serializes, in kebab case.
    pub name: &'static str,
    /// The payload's version.
    pub version: u32,
    /// Made by language producers from the syntax tree.
    pub syntactic: bool,
    /// Has a conformance inventory, `cupel-fixtures/conformance/<name>.toml` (design §8.3.7).
    pub conformance: bool,
    /// The `[lang.<id>]` key that pins this kind's producer, if one can be chosen.
    pub pin_key: Option<&'static str>,
    /// The payload's JSON Schema.
    pub schema: fn() -> Schema,
    /// The payload read from JSON, with its type erased, for producers whose fact arrives as JSON
    /// (producer plugins, ADR-089).
    pub decode: fn(Value) -> serde_json::Result<Produced>,
}

inventory::collect!(FactEntry);

impl FactEntry {
    /// The entry of an execution fact `F`, made by tools and engines, with no conformance
    /// inventory and no pin key.
    pub const fn execution<F: Fact>(name: &'static str) -> Self {
        Self {
            kind: F::KIND,
            name,
            version: F::VERSION,
            syntactic: false,
            conformance: false,
            pin_key: None,
            schema: crate::schema::fact_schema::<F>,
            decode: decode::<F>,
        }
    }

    /// The entry of a syntactic fact `F`, made by language producers and checked against its
    /// conformance inventory.
    pub const fn syntactic<F: Fact>(name: &'static str) -> Self {
        Self {
            syntactic: true,
            conformance: true,
            ..Self::execution::<F>(name)
        }
    }

    /// This entry, with a conformance inventory its tool producers' tests check (ADR-116).
    #[must_use]
    pub const fn with_conformance(self) -> Self {
        Self {
            conformance: true,
            ..self
        }
    }

    /// This entry, with its producer pinned by `[lang.<id>] <key> = "<producer>"`.
    #[must_use]
    pub const fn pinned_by(self, key: &'static str) -> Self {
        Self {
            pin_key: Some(key),
            ..self
        }
    }

    /// Every entry, in catalog order (`FactKind`'s declaration order, whatever the link order).
    ///
    /// # Panics
    ///
    /// When two entries are submitted for one kind.
    pub fn catalog() -> &'static [&'static FactEntry] {
        &catalog().entries
    }
}

fn decode<F: Fact>(value: Value) -> serde_json::Result<Produced> {
    serde_json::from_value::<F>(value).map(Produced::new)
}

/// The submitted entries, sorted, and the kind lists derived from them.
struct Catalog {
    entries: Vec<&'static FactEntry>,
    kinds: Vec<FactKind>,
    syntactic: Vec<FactKind>,
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut entries: Vec<&'static FactEntry> =
            inventory::iter::<FactEntry>.into_iter().collect();
        entries.sort_by_key(|entry| entry.kind);
        if let Some(pair) = entries.windows(2).find(|pair| pair[0].kind == pair[1].kind) {
            panic!(
                "fact kinds have one catalog entry each: `{}` is submitted twice",
                pair[0].name
            );
        }
        let kinds = entries.iter().map(|entry| entry.kind).collect();
        let syntactic = entries
            .iter()
            .filter(|entry| entry.syntactic)
            .map(|entry| entry.kind)
            .collect();
        Catalog {
            entries,
            kinds,
            syntactic,
        }
    })
}
