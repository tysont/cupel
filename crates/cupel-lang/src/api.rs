// ABOUTME: `QueryApi`: public items from api.scm with paths and signatures normalized for comparison
// ABOUTME: (parameter names become `_`, layout and trailing commas disappear).
//!
//! # `api.scm`
//!
//! - `@api.function`, `@api.method`, `@api.type`, `@api.interface`, `@api.constant`,
//!   `@api.variable`, or `@api.module` on a public item's node; the capture name is its kind.
//!   The earliest pattern wins for an item node.
//! - `@api.name` on the node whose text names it. Its path is the namespace (file namespace,
//!   enclosing `@scope`s, as for symbols) joined with the name.
//! - `@api.signature` (optional, repeatable) on the nodes that make up its signature: the
//!   signature is every token from the start of the first to the end of the last, comments
//!   skipped, so text between them (such as `->`) is kept.
//! - `@api.local` (any match) on nodes whose text is a local name, such as parameter patterns;
//!   inside a signature each becomes `_`.
//! - `@root.entry-point` and `@root.hook` belong to `QueryRoots` (see `cupel_lang::roots`).
//!
//! Tokens are joined by single spaces, except none before `, ) ] > : ; ? .` and `::`, none
//! after `( [ < & * ' .` and `::`, and none before an opening bracket that follows a name; a
//! comma right before a closing bracket or at the end is dropped.
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
    ApiSurface {
        items: items
            .into_iter()
            .filter_map(|(m, kind, node)| {
                let name = parsed.text(m.node(capture::API_NAME)?);
                let namespace = namespaces.of(node, &none, None);
                let signature_nodes: Vec<Node<'_>> = m.nodes(capture::API_SIGNATURE).collect();
                Some(ApiItem {
                    path: namespaces.display(&namespace, name),
                    kind,
                    signature: signature(parsed, node, &signature_nodes, &locals),
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

/// The normalized signature spanning `nodes`, read from the leaves of `item`.
fn signature(parsed: &Parsed, item: Node<'_>, nodes: &[Node<'_>], locals: &[Node<'_>]) -> String {
    let (Some(start), Some(end)) = (
        nodes.iter().map(Node::start_byte).min(),
        nodes.iter().map(Node::end_byte).max(),
    ) else {
        return String::new();
    };
    let mut tokens: Vec<String> = Vec::new();
    let mut last_local = None;
    leaves(item, &mut |leaf| {
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
                tokens.push("_".to_owned());
            }
            None => {
                last_local = None;
                tokens.push(parsed.text(leaf).to_owned());
            }
        }
    });
    join(&tokens)
}

/// Calls `visit` on every leaf below `node`, in document order, skipping comments.
fn leaves<'t>(node: Node<'t>, visit: &mut dyn FnMut(Node<'t>)) {
    if node.is_extra() {
        return;
    }
    if node.child_count() == 0 {
        visit(node);
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        leaves(child, visit);
    }
}

/// Tokens joined as the module docs describe.
fn join(tokens: &[String]) -> String {
    let closes = |t: &str| matches!(t, ")" | "]" | ">" | "}");
    let mut out = String::new();
    let mut previous: Option<&str> = None;
    for (index, token) in tokens.iter().enumerate() {
        let token = token.as_str();
        let next = tokens.get(index + 1).map(String::as_str);
        if token == "," && next.is_none_or(closes) {
            continue;
        }
        if let Some(prev) = previous {
            let tight_before =
                matches!(token, "," | ")" | "]" | ">" | ":" | ";" | "?" | "." | "::");
            let tight_after = matches!(prev, "(" | "[" | "<" | "&" | "*" | "'" | "." | "::");
            let call_like = matches!(token, "(" | "<" | "[")
                && prev
                    .chars()
                    .last()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '>');
            if !(tight_before || tight_after || call_like) {
                out.push(' ');
            }
        }
        out.push_str(token);
        previous = Some(token);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::join;

    fn joined(tokens: &[&str]) -> String {
        join(&tokens.iter().map(|t| (*t).to_owned()).collect::<Vec<_>>())
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
}
