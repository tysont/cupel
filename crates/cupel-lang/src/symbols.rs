// ABOUTME: `QuerySymbols`: functions, methods, nested functions, and named closures from symbols.scm,
// ABOUTME: with namespace, visibility, and spans. Other producers attach their facts to these symbols.
//!
//! # `symbols.scm`
//!
//! Each pattern matches one symbol definition:
//!
//! - `@symbol.function`, `@symbol.method`, or `@symbol.closure` on the node whose span is the
//!   symbol's span; the capture name is the symbol's kind. A method is any function defined on a
//!   type or class, with or without a receiver. A closure is an anonymous function bound to a
//!   name, and its node is the binding (`let f = |x| …`, `const f = () => …`).
//! - `@symbol.name` on the node whose text is the symbol's name.
//! - `@symbol.body` (optional) on the body.
//! - `@symbol.visibility` (optional) on a modifier whose text `[visibility]` maps.
//! - `@visibility.public`, `@visibility.internal`, or `@visibility.private` (optional, on any
//!   node of the match) fixes the visibility outright, such as for trait-impl methods.
//! - `@scope.name` (optional) inside a symbol's match adds one innermost namespace segment that
//!   is not lexical, such as a Go method's receiver type.
//!
//! When several patterns match the same definition node, **the earliest pattern in the file
//! wins**, so specific patterns (methods in trait impls) come before general ones (every
//! function). Patterns without a symbol capture declare namespaces: `@scope` on a node whose
//! contents are namespaced by the text of its `@scope.name` (an `impl` block, a class, an
//! inline module), and `@namespace.name` on a node whose text replaces the path-derived file
//! namespace (Go's package clause). A symbol's namespace is the file namespace, then every
//! scope and every other symbol that encloses it, outermost first, joined by
//! `[namespace].separator`; nested functions and closures are therefore namespaced by the
//! function they are defined in.
use std::collections::BTreeMap;

use cupel_core::syntax::{SymbolDef, SymbolKind, Symbols, Visibility};
use tree_sitter::Node;

use crate::contract::capture;
use crate::language::Language;
use crate::namespace::{Namespaces, Scope};
use crate::parse::Parsed;
use crate::producer::Source;
use crate::query::{Match, matches, span};
use crate::spec::QueryFile;

/// A symbol and the nodes it was read from, for producers that attach facts to symbols.
#[derive(Debug, Clone)]
pub struct Definition<'t> {
    pub node: Node<'t>,
    pub name: Node<'t>,
    pub body: Option<Node<'t>>,
    pub symbol: SymbolDef,
}

/// The `Symbols` extractor.
pub fn extract(source: &Source<'_>) -> Symbols {
    Symbols {
        symbols: definitions(source.parsed, source.language)
            .into_iter()
            .map(|definition| definition.symbol)
            .collect(),
    }
}

/// Every symbol in `parsed`, in document order.
///
/// # Panics
///
/// When the language's `symbols.scm` does not compile; a test compiles every query.
pub fn definitions<'t>(parsed: &'t Parsed, language: &dyn Language) -> Vec<Definition<'t>> {
    let spec = language.spec();
    let query = spec
        .query(QueryFile::Symbols, &parsed.grammar)
        .unwrap_or_else(|error| panic!("{} symbols.scm: {error}", language.id()));
    let found = matches(&query, parsed);
    let namespaces = Namespaces::new(&spec.tables().namespace, parsed, &found);

    // The earliest pattern wins for each definition node.
    let mut chosen: BTreeMap<usize, (&Match<'_, 't>, SymbolKind, Node<'t>)> = BTreeMap::new();
    for m in &found {
        let Some((kind, node)) = definition_capture(m) else {
            continue;
        };
        if m.node(capture::SYMBOL_NAME).is_none() {
            continue;
        }
        let entry = chosen.entry(node.id()).or_insert((m, kind, node));
        if m.pattern < entry.0.pattern {
            *entry = (m, kind, node);
        }
    }
    let mut picked: Vec<(&Match<'_, 't>, SymbolKind, Node<'t>)> = chosen.into_values().collect();
    picked.sort_by_key(|(_, _, node)| (node.start_byte(), std::cmp::Reverse(node.end_byte())));

    // A symbol namespaces what it encloses by its name and any non-lexical segment of its
    // own (a Go receiver type), so a closure in a method is `Type.method.closure`.
    let enclosing: Vec<Scope<'t>> = picked
        .iter()
        .filter_map(|(m, _, node)| {
            let name = parsed.text(m.node(capture::SYMBOL_NAME)?);
            let extra = m.node(capture::SCOPE_NAME).map_or("", |n| parsed.text(n));
            Some(Scope {
                node: *node,
                name: namespaces.join(extra, name),
            })
        })
        .collect();
    let rules = &spec.tables().visibility;
    picked
        .into_iter()
        .filter_map(|(m, kind, node)| {
            let name_node = m.node(capture::SYMBOL_NAME)?;
            let name = parsed.text(name_node);
            let extra = m.node(capture::SCOPE_NAME).map(|n| parsed.text(n));
            let namespace = namespaces.of(node, &enclosing, extra);
            let visibility = fixed_visibility(m).unwrap_or_else(|| {
                rules.resolve(
                    name,
                    m.node(capture::SYMBOL_VISIBILITY).map(|n| parsed.text(n)),
                )
            });
            let body = m.node(capture::SYMBOL_BODY);
            Some(Definition {
                node,
                name: name_node,
                body,
                symbol: SymbolDef {
                    name: name.to_owned(),
                    display: namespaces.display(&namespace, name),
                    namespace,
                    kind,
                    visibility,
                    lang: language.id(),
                    file: parsed.file.clone(),
                    span: span(node),
                    body: body.map(span),
                },
            })
        })
        .collect()
}

fn definition_capture<'t>(m: &Match<'_, 't>) -> Option<(SymbolKind, Node<'t>)> {
    [
        (capture::SYMBOL_FUNCTION, SymbolKind::Function),
        (capture::SYMBOL_METHOD, SymbolKind::Method),
        (capture::SYMBOL_CLOSURE, SymbolKind::Closure),
    ]
    .into_iter()
    .find_map(|(name, kind)| m.node(name).map(|node| (kind, node)))
}

fn fixed_visibility(m: &Match<'_, '_>) -> Option<Visibility> {
    [
        (capture::VISIBILITY_PUBLIC, Visibility::Public),
        (capture::VISIBILITY_INTERNAL, Visibility::Internal),
        (capture::VISIBILITY_PRIVATE, Visibility::Private),
    ]
    .into_iter()
    .find_map(|(name, visibility)| m.has(name).then_some(visibility))
}
