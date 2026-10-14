// ABOUTME: `QueryMutationSites`: places a mutation operator applies, per symbol, from mutations.scm,
// ABOUTME: with replacement spellings from `[mutations]` and a structural `site_hash` that ignores lines.
//!
//! # `mutations.scm`
//!
//! Each `@mut.<operator>` capture is one site: the node whose text is replaced (an operator
//! token, a literal, a statement, a returned value, a condition). `<operator>` is one of
//! `relational`, `equality`, `arith-add-sub`, `arith-mul-div`, `boolean-op`,
//! `boolean-literal`, `zero-one` (the `default` set) or `statement-deletion`, `return-value`,
//! `constant`, `boundary`, `negation` (the `extended` set); the set follows from the operator.
//! `[mutations.<operator>]` gives the replacements. A site belongs to the innermost symbol
//! that contains it; every symbol gets an entry. Sites are ordered by position, outer before
//! inner, then by operator.
//!
//! `site_hash` is the first 64 bits of the BLAKE3 digest of the operator and the site's path
//! from the symbol's definition node: each step's node kind and index among its parent's
//! children, comments excluded. It survives moving the symbol and reformatting it.
use std::collections::HashSet;
use std::fmt::Write as _;

use cupel_core::SiteHash;
use cupel_core::syntax::{MutationOperator, MutationSites, OperatorSet, Site, SymbolSites};
use serde_json::Value;
use tree_sitter::Node;

use crate::contract::capture;
use crate::normalize::hex16;
use crate::parse::Parsed;
use crate::producer::Source;
use crate::query::{matches, span};
use crate::spec::QueryFile;
use crate::symbols::{Definition, definitions};

/// The `MutationSites` extractor.
///
/// # Panics
///
/// When the language's `mutations.scm` does not compile; a test compiles every query.
pub fn extract(source: &Source<'_>) -> MutationSites {
    let parsed = source.parsed;
    let spec = source.language.spec();
    let symbols = definitions(parsed, source.language);
    let query = spec
        .query(QueryFile::Mutations, &parsed.grammar)
        .unwrap_or_else(|error| panic!("{} mutations.scm: {error}", source.language.id()));
    let mut seen = HashSet::new();
    let mut found: Vec<(MutationOperator, Node<'_>)> = matches(&query, parsed)
        .iter()
        .flat_map(|m| m.captures.clone())
        .filter_map(|(name, node)| {
            let operator = name.strip_prefix(capture::MUT_PREFIX)?;
            let operator: MutationOperator =
                serde_json::from_value(Value::String(operator.to_owned())).ok()?;
            seen.insert((node.id(), operator))
                .then_some((operator, node))
        })
        .collect();
    found.sort_by_key(|(operator, node)| {
        (
            node.start_byte(),
            std::cmp::Reverse(node.end_byte()),
            *operator as u8,
        )
    });
    MutationSites {
        symbols: symbols
            .iter()
            .map(|symbol| SymbolSites {
                symbol: symbol.symbol.key(),
                sites: found
                    .iter()
                    .filter(|(_, node)| {
                        innermost(&symbols, *node).is_some_and(|s| s.node.id() == symbol.node.id())
                    })
                    .filter_map(|(operator, node)| {
                        site(parsed, symbol, *operator, *node, spec.tables())
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn innermost<'a, 't>(symbols: &'a [Definition<'t>], node: Node<'_>) -> Option<&'a Definition<'t>> {
    symbols
        .iter()
        .filter(|s| {
            s.node.start_byte() <= node.start_byte() && node.end_byte() <= s.node.end_byte()
        })
        .min_by_key(|s| s.node.end_byte() - s.node.start_byte())
}

fn site(
    parsed: &Parsed,
    symbol: &Definition<'_>,
    operator: MutationOperator,
    node: Node<'_>,
    tables: &crate::tables::Tables,
) -> Option<Site> {
    let from = parsed.text(node);
    let to = tables.mutations.get(&operator)?.of(from);
    if to.is_empty() {
        return None;
    }
    Some(Site {
        operator,
        set: set_of(operator),
        from: from.to_owned(),
        to,
        site_hash: site_hash(operator, symbol.node, node),
        span: span(node),
    })
}

/// The set an operator belongs to (design §8.4.3).
pub fn set_of(operator: MutationOperator) -> OperatorSet {
    match operator {
        MutationOperator::Relational
        | MutationOperator::Equality
        | MutationOperator::ArithAddSub
        | MutationOperator::ArithMulDiv
        | MutationOperator::BooleanOp
        | MutationOperator::BooleanLiteral
        | MutationOperator::ZeroOne => OperatorSet::Default,
        MutationOperator::StatementDeletion
        | MutationOperator::ReturnValue
        | MutationOperator::Constant
        | MutationOperator::Boundary
        | MutationOperator::Negation => OperatorSet::Extended,
    }
}

/// The operator and the path from `root` down to `node`, hashed.
fn site_hash(operator: MutationOperator, root: Node<'_>, node: Node<'_>) -> SiteHash {
    let mut steps = Vec::new();
    let mut current = node;
    while current.id() != root.id() {
        let Some(parent) = current.parent() else {
            break;
        };
        let mut cursor = parent.walk();
        let index = parent
            .children(&mut cursor)
            .filter(|child| !child.is_extra())
            .position(|child| child.id() == current.id())
            .unwrap_or(usize::MAX);
        steps.push((current.kind(), index));
        current = parent;
    }
    let mut path = serde_json::to_string(&operator).unwrap_or_default();
    for (kind, index) in steps.iter().rev() {
        let _ = write!(path, "/{kind}:{index}");
    }
    hex16(blake3::hash(path.as_bytes()).as_bytes())
        .parse()
        .expect("sixteen hex digits make a valid site hash")
}
