// ABOUTME: `QueryDecisions`: decision points per symbol from decisions.scm, with nesting and the
// ABOUTME: cyclomatic and cognitive weights from `[decisions]`. Computing complexity is a check's job.
//!
//! # `decisions.scm`
//!
//! Each `@decision.<kind>` capture is one decision, `<kind>` one of `branch`, `loop`,
//! `boolean-op`, `case`, `catch`, `early-return`, `ternary`. Capture the construct's own node:
//! the `if`, the loop, the binary expression, the match arm or case clause (not default arms),
//! the handler, the `return`. A decision belongs to the innermost symbol that contains it, so
//! decisions in a nested function or a named closure are that symbol's; anonymous closures
//! passed inline stay with their enclosing function.
//!
//! - **Nesting** counts the enclosing decisions of nesting kinds within the symbol, except an
//!   enclosing decision reached through one of its `condition_fields` (a `&&` in an `if`
//!   condition is not inside the `if`), and except the branch an else-if continues: a branch
//!   whose parent node's kind is in `chain_parents` is not nested under the branch that owns
//!   that parent, and neither is anything nested under it.
//! - **Early returns** whose node is the symbol body's final statement are dropped: a return
//!   is final when every node from it up to the body is its parent's last named child. A return
//!   that ends an anonymous function inside the symbol (one of `[decisions] functions`, a
//!   callback passed inline) is final there, since it leaves only that function.
//! - **Cases**: only the first `case` among siblings keeps its cognitive weight; the others
//!   are marked `continues` (one multiway decision, ADR-072).
//! - **Tables**: `@loop.table` (any pattern) on a loop node marks it as iterating over a table
//!   of cases written in place; the loop's decision has `table: true` (ADR-069).
use std::collections::HashSet;

use cupel_core::FactKind;
use cupel_core::syntax::{Decision, DecisionKind, Decisions, SymbolDecisions};
use serde_json::Value;
use tree_sitter::Node;

use crate::contract::capture;
use crate::producer::{QueryProducer, Source};
use crate::query::{matches, span};
use crate::spec::QueryFile;
use crate::symbols::{Definition, definitions};
use crate::tables::DecisionRules;

inventory::submit!(crate::generic::GenericProducer {
    kind: FactKind::Decisions,
    drives: |spec| spec.has(QueryFile::Symbols) && spec.has(QueryFile::Decisions),
    attach: |language, cache, reg| {
        reg.register::<Decisions, _>(QueryProducer::new(language, cache, extract));
    },
});

/// The `Decisions` extractor.
///
/// # Panics
///
/// When the language's `decisions.scm` does not compile; a test compiles every query.
pub fn extract(source: &Source<'_>) -> Decisions {
    let parsed = source.parsed;
    let spec = source.language.spec();
    let rules = &spec.tables().decisions;
    let symbols = definitions(parsed, source.language);
    let query = spec
        .query(QueryFile::Decisions, &parsed.grammar)
        .unwrap_or_else(|error| panic!("{} decisions.scm: {error}", source.language.id()));
    let mut seen = HashSet::new();
    let all = matches(&query, parsed);
    let mut found: Vec<(DecisionKind, Node<'_>)> = all
        .iter()
        .filter_map(|m| {
            let (kind, node) = m.prefixed(capture::DECISION_PREFIX)?;
            let kind: DecisionKind = serde_json::from_value(Value::String(kind.to_owned())).ok()?;
            seen.insert((node.id(), kind)).then_some((kind, node))
        })
        .collect();
    found.sort_by_key(|(_, node)| node.start_byte());
    let tables: HashSet<usize> = all
        .iter()
        .flat_map(|m| m.nodes(capture::LOOP_TABLE))
        .map(|node| node.id())
        .collect();

    let owners: Vec<Option<usize>> = found
        .iter()
        .map(|(_, node)| innermost(&symbols, *node))
        .collect();
    Decisions {
        symbols: symbols
            .iter()
            .enumerate()
            .map(|(index, symbol)| {
                let mine: Vec<(DecisionKind, Node<'_>)> = found
                    .iter()
                    .zip(&owners)
                    .filter(|(_, owner)| **owner == Some(index))
                    .map(|(decision, _)| *decision)
                    .collect();
                SymbolDecisions {
                    symbol: symbol.symbol.key(),
                    decisions: weigh(&mine, symbol, rules, &tables),
                }
            })
            .collect(),
    }
}

/// The index of the smallest symbol whose node contains `node`.
fn innermost(symbols: &[Definition<'_>], node: Node<'_>) -> Option<usize> {
    symbols
        .iter()
        .enumerate()
        .filter(|(_, symbol)| {
            symbol.node.start_byte() <= node.start_byte()
                && node.end_byte() <= symbol.node.end_byte()
        })
        .min_by_key(|(_, symbol)| symbol.node.end_byte() - symbol.node.start_byte())
        .map(|(index, _)| index)
}

fn weigh(
    found: &[(DecisionKind, Node<'_>)],
    symbol: &Definition<'_>,
    rules: &DecisionRules,
    tables: &HashSet<usize>,
) -> Vec<Decision> {
    let body = symbol.body.unwrap_or(symbol.node);
    let kept: Vec<(DecisionKind, Node<'_>)> = found
        .iter()
        .copied()
        .filter(|(kind, node)| {
            *kind != DecisionKind::EarlyReturn || !is_final(*node, body, &rules.functions)
        })
        .collect();
    let mut cased_parents = HashSet::new();
    kept.iter()
        .map(|(kind, node)| {
            let weight = rules.weight(*kind);
            let first_case = *kind != DecisionKind::Case
                || node.parent().is_none_or(|p| cased_parents.insert(p.id()));
            Decision {
                kind: *kind,
                nesting: nesting(*node, symbol.node, &kept, rules),
                cyclomatic: weight.cyclomatic,
                cognitive: if first_case { weight.cognitive } else { 0 },
                table: *kind == DecisionKind::Loop && tables.contains(&node.id()),
                continues: !first_case,
                span: Some(span(*node)),
            }
        })
        .collect()
}

/// How many enclosing decisions of nesting kinds `node` is inside, within `owner`.
fn nesting(
    node: Node<'_>,
    owner: Node<'_>,
    decisions: &[(DecisionKind, Node<'_>)],
    rules: &DecisionRules,
) -> u32 {
    let nests = |candidate: Node<'_>| {
        decisions
            .iter()
            .any(|(kind, n)| n.id() == candidate.id() && rules.weight(*kind).nests)
    };
    let is_chain_link = |candidate: Node<'_>| {
        candidate
            .parent()
            .is_some_and(|parent| rules.chain_parents.iter().any(|k| k == parent.kind()))
    };
    let mut depth = 0;
    let mut skip_chain = is_chain_link(node);
    let mut child = node;
    while let Some(parent) = child.parent() {
        if parent.id() == owner.id() {
            break;
        }
        if nests(parent) {
            let in_condition = rules.condition_fields.iter().any(|field| {
                parent
                    .child_by_field_name(field)
                    .is_some_and(|c| c.id() == child.id())
            });
            if !skip_chain && !in_condition {
                depth += 1;
            }
            skip_chain = is_chain_link(parent);
        }
        child = parent;
    }
    depth
}

/// Whether `node` is the final statement of `body`: every node from it up to `body` is its
/// parent's last named child, ignoring comments.
fn is_final(node: Node<'_>, body: Node<'_>, functions: &[String]) -> bool {
    let mut child = node;
    while let Some(parent) = child.parent() {
        let mut cursor = parent.walk();
        let last = parent
            .named_children(&mut cursor)
            .filter(|n| !n.is_extra())
            .last();
        if last.is_none_or(|last| last.id() != child.id()) {
            return false;
        }
        if parent.id() == body.id() || functions.iter().any(|kind| kind == parent.kind()) {
            return true;
        }
        child = parent;
    }
    false
}
