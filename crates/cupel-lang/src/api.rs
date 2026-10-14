// ABOUTME: `QueryApi`: public items from api.scm with paths and signatures normalized for comparison
// ABOUTME: (parameter names become `_`, layout and trailing commas disappear).
//!
//! # `api.scm`
//!
//! - `@api.function`, `@api.method`, `@api.type`, `@api.interface`, `@api.constant`,
//!   `@api.variable`, or `@api.module` on a public item's node; the capture name is its kind.
//!   The earliest pattern wins for an item node.
//! - `@api.name` on the node whose text names it. Its path is the namespace (file namespace,
//!   enclosing `@scope`s, as for symbols) joined with the name; an `@scope.name` in the item's
//!   own match adds one innermost segment that is not lexical, such as a Go method's receiver
//!   type, as it does in symbols.scm.
//! - `@api.signature` (optional, repeatable) on the nodes that make up its signature: the
//!   signature is every token from the start of the first to the end of the last, comments
//!   skipped, so text between them (such as `->`) is kept.
//! - `@api.local` (any match) on nodes whose text is a local name, such as parameter patterns;
//!   inside a signature each becomes `_`.
//! - `@root.entry-point` and `@root.hook` belong to `QueryRoots` (see `cupel_lang::roots`).
//!
//! A node whose kind is in `[normalize].literals` (a string literal, say) is one token, its
//! source text, so quotes stay attached to what they quote. Tokens are joined by single spaces, except none before `, ) ] > : ; ? .` and `::`, none
//! after `( [ < & * ' .` and `::` or after a token in `[api].tight_after`, and none before an
//! opening bracket that follows a name (the `_` standing for a local is not a name); a comma
//! right before a closing bracket or at the end is dropped.
use std::collections::BTreeMap;

use cupel_core::syntax::{ApiItem, ApiItemKind, ApiSurface};
use serde_json::Value;
use tree_sitter::Node;

use crate::contract::capture;
use crate::namespace::{Namespaces, Scope};
use crate::parse::Parsed;
use crate::producer::Source;
use crate::query::{Match, matches, span};
use crate::spec::QueryFile;

/// What a local name becomes in a signature.
const LOCAL: &str = "_";

/// The `ApiSurface` extractor.
///
/// # Panics
///
/// When the language's `api.scm` does not compile; a test compiles every query.
pub fn extract(source: &Source<'_>) -> ApiSurface {
    let parsed = source.parsed;
    let spec = source.language.spec();
    let query = spec
        .query(QueryFile::Api, &parsed.grammar)
        .unwrap_or_else(|error| panic!("{} api.scm: {error}", source.language.id()));
    let found = matches(&query, parsed);
    let namespaces = Namespaces::new(&spec.tables().namespace, parsed, &found);
    let locals: Vec<Node<'_>> = found
        .iter()
        .flat_map(|m| m.nodes(capture::API_LOCAL).collect::<Vec<_>>())
        .collect();

    let mut chosen: BTreeMap<usize, (&Match<'_, '_>, ApiItemKind, Node<'_>)> = BTreeMap::new();
    for m in &found {
        let Some((kind, node)) = item_capture(m) else {
            continue;
        };
        let entry = chosen.entry(node.id()).or_insert((m, kind, node));
        if m.pattern < entry.0.pattern {
            *entry = (m, kind, node);
        }
    }
    let mut items: Vec<(&Match<'_, '_>, ApiItemKind, Node<'_>)> = chosen.into_values().collect();
    items.sort_by_key(|(_, _, node)| node.start_byte());
    let none: [Scope<'_>; 0] = [];
    let tight_after = &spec.tables().api.tight_after;
    let literals = spec
        .tables()
        .normalize
        .as_ref()
        .map_or(&[][..], |rules| rules.literals.as_slice());
    ApiSurface {
        items: items
            .into_iter()
            .filter_map(|(m, kind, node)| {
                let name = parsed.text(m.node(capture::API_NAME)?);
                let extra = m.node(capture::SCOPE_NAME).map(|n| parsed.text(n));
                let namespace = namespaces.of(node, &none, extra);
                let signature_nodes: Vec<Node<'_>> = m.nodes(capture::API_SIGNATURE).collect();
                Some(ApiItem {
                    path: namespaces.display(&namespace, name),
                    kind,
                    signature: signature(
                        parsed,
                        node,
                        &signature_nodes,
                        &locals,
                        Spelling {
                            tight_after,
                            literals,
                        },
                    ),
                    file: parsed.file.clone(),
                    span: span(node),
                })
            })
            .collect(),
    }
}

fn item_capture<'t>(m: &Match<'_, 't>) -> Option<(ApiItemKind, Node<'t>)> {
    let (kind, node) = m.captures.iter().find_map(|(name, node)| {
        let kind = name.strip_prefix(capture::API_PREFIX)?;
        let kind: ApiItemKind = serde_json::from_value(Value::String(kind.to_owned())).ok()?;
        Some((kind, *node))
    })?;
    Some((kind, node))
}

/// The language's tables that shape a signature's tokens.
#[derive(Clone, Copy)]
struct Spelling<'a> {
    /// Tokens with no space after them.
    tight_after: &'a [String],
    /// Node kinds read as one token.
    literals: &'a [String],
}

/// The normalized signature spanning `nodes`, read from the tokens of `item`.
fn signature(
    parsed: &Parsed,
    item: Node<'_>,
    nodes: &[Node<'_>],
    locals: &[Node<'_>],
    spelling: Spelling<'_>,
) -> String {
    let (Some(start), Some(end)) = (
        nodes.iter().map(Node::start_byte).min(),
        nodes.iter().map(Node::end_byte).max(),
    ) else {
        return String::new();
    };
    let mut tokens: Vec<String> = Vec::new();
    let mut last_local = None;
    tokens_of(item, spelling.literals, &mut |leaf| {
        if leaf.start_byte() < start || leaf.end_byte() > end {
            return;
        }
        let local = locals.iter().find(|local| {
            local.start_byte() <= leaf.start_byte() && leaf.end_byte() <= local.end_byte()
        });
        match local {
            Some(local) if last_local == Some(local.id()) => {}
            Some(local) => {
                last_local = Some(local.id());
                tokens.push(LOCAL.to_owned());
            }
            None => {
                last_local = None;
                tokens.push(parsed.text(leaf).to_owned());
            }
        }
    });
    join(&tokens, spelling.tight_after)
}

/// Calls `visit` on every token below `node`, in document order, skipping comments: a leaf, or
/// a node whose kind is one of `literals`.
fn tokens_of<'t>(node: Node<'t>, literals: &[String], visit: &mut dyn FnMut(Node<'t>)) {
    if node.is_extra() {
        return;
    }
    if node.child_count() == 0 || literals.iter().any(|kind| kind == node.kind()) {
        visit(node);
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        tokens_of(child, literals, visit);
    }
}

/// Tokens joined as the module docs describe.
fn join(tokens: &[String], tight_after: &[String]) -> String {
    let closes = |t: &str| matches!(t, ")" | "]" | ">" | "}");
    let mut out = String::new();
    let mut previous: Option<&str> = None;
    for (index, token) in tokens.iter().enumerate() {
        let token = token.as_str();
        let next = tokens.get(index + 1).map(String::as_str);
        if token == "," && next.is_none_or(closes) {
            continue;
        }
        if previous.is_some_and(|prev| spaced(prev, token, tight_after)) {
            out.push(' ');
        }
        out.push_str(token);
        previous = Some(token);
    }
    out
}

/// Whether a space separates `prev` from the `token` after it: not before closing and joining
/// punctuation, not after opening punctuation or a `tight_after` token, and not between a
/// name and the bracket of a call or generic.
fn spaced(prev: &str, token: &str, tight_after: &[String]) -> bool {
    let tight_before = matches!(token, "," | ")" | "]" | ">" | ":" | ";" | "?" | "." | "::");
    let tight_after = matches!(prev, "(" | "[" | "<" | "&" | "*" | "'" | "." | "::")
        || tight_after.iter().any(|t| t == prev);
    let call_like = matches!(token, "(" | "<" | "[")
        && prev != LOCAL
        && prev
            .chars()
            .last()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '>');
    !(tight_before || tight_after || call_like)
}

#[cfg(test)]
mod tests {
    use super::join;

    fn joined(tokens: &[&str]) -> String {
        join(
            &tokens.iter().map(|t| (*t).to_owned()).collect::<Vec<_>>(),
            &[],
        )
    }

    #[test]
    fn tokens_join_with_conventional_spacing() {
        assert_eq!(
            joined(&["(", "_", ":", "&", "[", "i64", "]", ")", "->", "i64"]),
            "(_: &[i64]) -> i64"
        );
        assert_eq!(
            joined(&["(", "&", "mut", "self", ",", "_", ":", "i64", ",", ")"]),
            "(&mut self, _: i64)"
        );
        assert_eq!(
            joined(&["Option", "<", "&", "'", "static", "str", ">"]),
            "Option<&'static str>"
        );
        assert_eq!(
            joined(&["fmt", "::", "Formatter", "<", "'", "_", ">"]),
            "fmt::Formatter<'_>"
        );
        assert_eq!(joined(&["where", "T", ":", "Copy", ","]), "where T: Copy");
    }

    #[test]
    fn a_local_placeholder_is_not_a_name_and_tables_add_tight_tokens() {
        assert_eq!(joined(&["(", "_", "[", "]", "T", ")"]), "(_ [] T)");
        let tight = ["]".to_owned(), "...".to_owned()];
        let tokens: Vec<String> = ["(", "_", "[", "]", "T", ",", "_", "...", "U", ")"]
            .iter()
            .map(|t| (*t).to_owned())
            .collect();
        assert_eq!(join(&tokens, &tight), "(_ []T, _ ...U)");
    }
}
