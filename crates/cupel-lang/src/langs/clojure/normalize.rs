// ABOUTME: Clojure's NormTree: every form's shape with names and literals erased, except the function a
// ABOUTME: list calls (its head symbol), which keeps its name; tables cannot name a list's first element (ADR-109).
//!
//! A call in Clojure is a list whose first element is the function (`(+ a b)`, `(str/join x)`),
//! so what distinguishes `(+ a b)` from `(- a b)` and `(f x)` from `(g x)` is the text of that
//! first symbol. Every other symbol (parameters, locals, var references) becomes `$id`, as do
//! keywords (map keys play the role of field names); numbers, strings, characters, regular
//! expressions, booleans, and `nil` become `$lit`. Comments (ordinary nodes in this grammar,
//! not extras) and `#_` discarded forms are dropped, and so is the symbol's own name and the metadata on any symbol, so renaming a
//! function, marking it `^:private`, or reformatting it keeps its hash.
use std::sync::Arc;

use cupel_core::ProducerRegistry;
use cupel_core::syntax::{NormNode, NormTree, NormalizedSymbol};
use tree_sitter::Node;

use crate::language::Language;
use crate::normalize::structural_hash;
use crate::parse::{ParseCache, Parsed};
use crate::producer::{QueryProducer, Source};
use crate::symbols::definitions;

/// What a name that is not a call's function becomes.
const NAME: &str = "$id";
/// What a literal becomes.
const LITERAL: &str = "$lit";

/// Registers Clojure's `NormTree` producer, which no `[normalize]` table drives.
pub fn register(language: &Arc<dyn Language>, cache: &Arc<ParseCache>, reg: &mut ProducerRegistry) {
    reg.register::<NormTree, _>(QueryProducer::new(
        Arc::clone(language),
        Arc::clone(cache),
        extract,
    ));
}

/// The `NormTree` extractor for Clojure.
pub fn extract(source: &Source<'_>) -> NormTree {
    let parsed = source.parsed;
    let lang = source.language.id();
    NormTree {
        symbols: definitions(parsed, source.language)
            .iter()
            .map(|definition| {
                let normalizer = Normalizer {
                    parsed,
                    skip: definition.name.id(),
                };
                let mut nodes = normalizer.node(definition.node, false);
                let root = if nodes.len() == 1 {
                    nodes.remove(0)
                } else {
                    NormNode::new(definition.node.kind(), nodes)
                };
                NormalizedSymbol {
                    symbol: definition.symbol.key(),
                    hash: structural_hash(&lang, &root),
                    root,
                }
            })
            .collect(),
    }
}

struct Normalizer<'a> {
    parsed: &'a Parsed,
    /// The `sym_name` of the symbol's own name, dropped with the symbol around it.
    skip: usize,
}

impl Normalizer<'_> {
    /// The normalized form of `node`, which is a list's first element when `head`.
    fn node(&self, node: Node<'_>, head: bool) -> Vec<NormNode> {
        if !node.is_named() || matches!(node.kind(), "comment" | "dis_expr") {
            return Vec::new();
        }
        match node.kind() {
            "sym_lit" if self.is_own_name(node) => Vec::new(),
            "sym_lit" if head => vec![NormNode::leaf(&self.called(node))],
            "sym_lit" | "kwd_lit" => vec![NormNode::leaf(NAME)],
            "num_lit" | "str_lit" | "char_lit" | "regex_lit" | "bool_lit" | "nil_lit" => {
                vec![NormNode::leaf(LITERAL)]
            }
            "meta_lit" | "old_meta_lit" => Vec::new(),
            kind => {
                let mut children = Vec::new();
                let mut first = kind == "list_lit";
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    let form = self.node(child, first);
                    if child.is_named() && !matches!(child.kind(), "comment" | "dis_expr") {
                        first = false;
                    }
                    children.extend(form);
                }
                vec![NormNode::new(kind, children)]
            }
        }
    }

    /// Whether `symbol` is the definition's own name.
    fn is_own_name(&self, symbol: Node<'_>) -> bool {
        symbol
            .child_by_field_name("name")
            .is_some_and(|name| name.id() == self.skip)
    }

    /// The name a call's function symbol is written with, metadata left out: `f`, `str/join`.
    fn called(&self, symbol: Node<'_>) -> String {
        let text = |field: &str| {
            symbol
                .child_by_field_name(field)
                .map(|node| self.parsed.text(node))
        };
        match (text("namespace"), text("name")) {
            (Some(namespace), Some(name)) => format!("{namespace}/{name}"),
            (None, Some(name)) => name.to_owned(),
            _ => self.parsed.text(symbol).to_owned(),
        }
    }
}
