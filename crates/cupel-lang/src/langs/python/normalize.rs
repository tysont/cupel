// ABOUTME: Python's NormTree override: the table-driven tree with a method's `self`/`cls` parameter and a
// ABOUTME: function's docstring dropped, then rehashed, since neither says what the code does (ADR-034).
//!
//! Tables cannot express either rule. `self` is an ordinary identifier that only its position
//! in a method makes a receiver, and a docstring is an ordinary string statement that only its
//! position at the top of a body makes documentation. So `def f(self, x)` in a class hashes
//! like `def f(x)` with the same body, and adding a docstring keeps the hash.
use cupel_core::syntax::{NormNode, NormTree, SymbolKind};
use tree_sitter::Node;

use crate::normalize::{self, structural_hash};
use crate::parse::Parsed;
use crate::producer::Source;
use crate::symbols::definitions;

/// First parameters that name the instance or class a method is called on.
const RECEIVERS: [&str; 2] = ["self", "cls"];

/// The `NormTree` extractor for Python.
pub fn extract(source: &Source<'_>) -> NormTree {
    let mut tree = normalize::extract(source);
    let lang = source.language.id();
    let parsed = source.parsed;
    // Both lists hold the file's symbols in the same order; the key check guards that.
    for (normalized, definition) in tree
        .symbols
        .iter_mut()
        .zip(definitions(parsed, source.language))
    {
        if normalized.symbol != definition.symbol.key()
            || definition.node.kind() != "function_definition"
        {
            continue;
        }
        let root = &mut normalized.root;
        let mut changed = false;
        if definition.symbol.kind == SymbolKind::Method && has_receiver(parsed, definition.node) {
            changed |= drop_first_child(root, "parameters");
        }
        if definition.body.is_some_and(has_docstring) {
            changed |= drop_first_child(root, "block");
        }
        if changed {
            normalized.hash = structural_hash(&lang, root);
        }
    }
    tree
}

/// Whether the function's first parameter is `self` or `cls`, annotated or not.
fn has_receiver(parsed: &Parsed, function: Node<'_>) -> bool {
    let Some(first) = function
        .child_by_field_name("parameters")
        .and_then(|parameters| first_named(parameters))
    else {
        return false;
    };
    let name = match first.kind() {
        "identifier" => Some(first),
        "typed_parameter" => first_named(first).filter(|n| n.kind() == "identifier"),
        _ => None,
    };
    name.is_some_and(|name| RECEIVERS.contains(&parsed.text(name)))
}

/// Whether the body's first statement is a lone string: a docstring.
fn has_docstring(body: Node<'_>) -> bool {
    first_named(body).is_some_and(|statement| {
        statement.kind() == "expression_statement"
            && statement.named_child_count() == 1
            && first_named(statement)
                .is_some_and(|value| matches!(value.kind(), "string" | "concatenated_string"))
    })
}

/// The first named child of `node` that is not a comment.
fn first_named(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| !child.is_extra())
}

/// Removes the first child of `root`'s first child labeled `label`; whether one was removed.
fn drop_first_child(root: &mut NormNode, label: &str) -> bool {
    match root.children.iter_mut().find(|child| child.label == label) {
        Some(node) if !node.children.is_empty() => {
            node.children.remove(0);
            true
        }
        _ => false,
    }
}
