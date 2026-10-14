// ABOUTME: `QueryRoots`: symbols live by convention: entry points and hooks named in `[roots]` or
// ABOUTME: captured in api.scm, exported (public) symbols, and test cases from tests.scm.
use cupel_core::syntax::{LiveRoot, RootReason, Roots, SymbolKind, Visibility};
use tree_sitter::Node;

use crate::contract::capture;
use crate::producer::Source;
use crate::query::matches;
use crate::spec::QueryFile;
use crate::symbols::{Definition, definitions};

/// The `Roots` extractor. Registered only for languages with a `[roots]` table.
///
/// A capture applies to the symbol whose definition node it is, else to the innermost symbol
/// containing it. Each symbol is reported once per reason, reasons in the order entry point,
/// export, test, framework hook.
///
/// # Panics
///
/// When the language's `api.scm` or `tests.scm` does not compile; a test compiles every query.
pub fn extract(source: &Source<'_>) -> Roots {
    let parsed = source.parsed;
    let spec = source.language.spec();
    let Some(rules) = spec.tables().roots.as_ref() else {
        return Roots::default();
    };
    let symbols = definitions(parsed, source.language);
    let captured = |file: QueryFile, names: &[&str]| -> Vec<(String, Node<'_>)> {
        if !spec.has(file) {
            return Vec::new();
        }
        let query = spec
            .query(file, &parsed.grammar)
            .unwrap_or_else(|error| panic!("{} {file}: {error}", source.language.id()));
        matches(&query, parsed)
            .into_iter()
            .flat_map(|m| m.captures)
            .filter(|(name, _)| names.contains(name))
            .map(|(name, node)| (name.to_owned(), node))
            .collect()
    };
    let api = captured(
        QueryFile::Api,
        &[capture::ROOT_ENTRY_POINT, capture::ROOT_HOOK],
    );
    let tests = captured(QueryFile::Tests, &[capture::TEST_CASE]);

    let mut roots = Vec::new();
    for (index, definition) in symbols.iter().enumerate() {
        let symbol = &definition.symbol;
        let owns = |node: Node<'_>| owner(&symbols, node) == Some(index);
        let named = |names: &[String]| names.contains(&symbol.name);
        let reasons = [
            (
                RootReason::EntryPoint,
                (symbol.kind == SymbolKind::Function && named(&rules.entry_points))
                    || api
                        .iter()
                        .any(|(n, node)| n == capture::ROOT_ENTRY_POINT && owns(*node)),
            ),
            (RootReason::Export, symbol.visibility == Visibility::Public),
            (
                RootReason::Test,
                tests
                    .iter()
                    .any(|(_, node)| node.id() == definition.node.id()),
            ),
            (
                RootReason::FrameworkHook,
                named(&rules.hooks)
                    || api
                        .iter()
                        .any(|(n, node)| n == capture::ROOT_HOOK && owns(*node)),
            ),
        ];
        for (reason, applies) in reasons {
            if applies {
                roots.push(LiveRoot {
                    symbol: symbol.key(),
                    reason,
                });
            }
        }
    }
    Roots { roots }
}

/// The symbol whose node is `node`, else the innermost containing it.
fn owner(symbols: &[Definition<'_>], node: Node<'_>) -> Option<usize> {
    symbols
        .iter()
        .position(|s| s.node.id() == node.id())
        .or_else(|| {
            symbols
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    s.node.start_byte() <= node.start_byte() && node.end_byte() <= s.node.end_byte()
                })
                .min_by_key(|(_, s)| s.node.end_byte() - s.node.start_byte())
                .map(|(index, _)| index)
        })
}
