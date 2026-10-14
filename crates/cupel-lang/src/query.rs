// ABOUTME: Running a compiled query over a parsed file and reading its captures by name, plus
// ABOUTME: conversions from tree-sitter positions to the 1-based spans facts carry.
use cupel_core::syntax::Span;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use crate::parse::Parsed;

/// One match of one pattern: its captures, in capture order.
#[derive(Debug, Clone)]
pub struct Match<'q, 't> {
    /// The pattern's index in the query; earlier patterns come first in the file.
    pub pattern: usize,
    pub captures: Vec<(&'q str, Node<'t>)>,
}

impl<'t> Match<'_, 't> {
    /// The first node captured as `name`.
    pub fn node(&self, name: &str) -> Option<Node<'t>> {
        self.captures
            .iter()
            .find(|(capture, _)| *capture == name)
            .map(|(_, node)| *node)
    }

    /// Every node captured as `name`.
    pub fn nodes<'a>(&'a self, name: &'a str) -> impl Iterator<Item = Node<'t>> + 'a {
        self.captures
            .iter()
            .filter(move |(capture, _)| *capture == name)
            .map(|(_, node)| *node)
    }

    /// The first capture whose name starts with `prefix`, as (suffix, node).
    pub fn prefixed(&self, prefix: &str) -> Option<(&str, Node<'t>)> {
        self.captures.iter().find_map(|(capture, node)| {
            capture
                .strip_prefix(prefix)
                .filter(|suffix| !suffix.is_empty())
                .map(|suffix| (suffix, *node))
        })
    }

    /// Whether any capture is named `name`.
    pub fn has(&self, name: &str) -> bool {
        self.node(name).is_some()
    }
}

/// Every match of `query` in `parsed`, in the order tree-sitter reports them. Text predicates
/// (`#eq?`, `#match?`, `#any-of?`) are applied.
pub fn matches<'q, 't>(query: &'q Query, parsed: &'t Parsed) -> Vec<Match<'q, 't>> {
    let names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut found = Vec::new();
    let mut iter = cursor.matches(query, parsed.root(), parsed.source().as_bytes());
    while let Some(m) = iter.next() {
        found.push(Match {
            pattern: m.pattern_index,
            captures: m
                .captures()
                .iter()
                .map(|capture| (names[capture.index as usize], capture.node))
                .collect(),
        });
    }
    found
}

/// The node's span: 1-based line and byte column of its start, and of the position just after
/// its last byte.
pub fn span(node: Node<'_>) -> Span {
    let (start, end) = (node.start_position(), node.end_position());
    Span {
        line: one_based(start.row),
        col: one_based(start.column),
        end_line: one_based(end.row),
        end_col: one_based(end.column),
    }
}

/// A 0-based tree-sitter row or column as a 1-based number, saturating.
pub fn one_based(value: usize) -> u32 {
    u32::try_from(value).map_or(u32::MAX, |v| v.saturating_add(1))
}

/// Whether `outer` strictly contains `inner` (contains it and is not the same range).
pub fn encloses(outer: Node<'_>, inner: Node<'_>) -> bool {
    outer.start_byte() <= inner.start_byte()
        && inner.end_byte() <= outer.end_byte()
        && outer.byte_range() != inner.byte_range()
}
