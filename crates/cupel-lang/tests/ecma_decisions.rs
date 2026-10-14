// ABOUTME: The JavaScript and TypeScript Decisions producers: branches, loops, short-circuit operators,
// ABOUTME: switch cases, catches, ternaries, and early returns and throws, with nesting and weights.
mod support;

use std::path::Path;

use cupel_core::syntax::{DecisionKind, Decisions, Symbols};
use support::{fixture, loop_tables, produce, tree};

use DecisionKind::{BooleanOp, Branch, Case, Catch, EarlyReturn, Loop, Ternary};

/// (kind, nesting, cyclomatic, cognitive) for each decision of the symbol `display`.
fn decisions_of(lang: &str, root: &Path, display: &str) -> Vec<(DecisionKind, u32, u32, u32)> {
    let symbols: Symbols = produce(lang, root);
    let decisions: Decisions = produce(lang, root);
    let key = symbols
        .symbols
        .iter()
        .find(|s| s.display == display)
        .unwrap_or_else(|| panic!("no symbol {display}"))
        .key();
    decisions
        .symbols
        .iter()
        .find(|entry| entry.symbol == key)
        .unwrap_or_else(|| panic!("no decisions entry for {display}"))
        .decisions
        .iter()
        .map(|d| (d.kind, d.nesting, d.cyclomatic, d.cognitive))
        .collect()
}

/// Both fixture repositories: the TypeScript `decide.ts` is the JavaScript `decide.js` typed.
fn both(display: &str) -> [Vec<(DecisionKind, u32, u32, u32)>; 2] {
    ["javascript", "typescript"].map(|lang| decisions_of(lang, &fixture(lang), display))
}

#[test]
fn else_if_chains_are_flat_conditions_do_not_nest_and_final_returns_are_not_early() {
    for decisions in both("decide.classify") {
        assert_eq!(
            decisions,
            [
                (Branch, 0, 1, 1),
                (BooleanOp, 0, 1, 1),
                (EarlyReturn, 1, 0, 1),
                (Branch, 0, 1, 1),
                (EarlyReturn, 1, 0, 1),
                (Ternary, 0, 1, 1),
            ]
        );
    }
}

#[test]
fn a_return_inside_a_branch_inside_a_loop_is_nested_twice() {
    for decisions in both("decide.firstOverdraft") {
        assert_eq!(
            decisions,
            [(Loop, 0, 1, 1), (Branch, 1, 1, 1), (EarlyReturn, 2, 0, 1)]
        );
    }
}

#[test]
fn switch_cases_count_once_toward_cognitive_and_default_is_not_a_case() {
    for decisions in both("decide.label") {
        assert_eq!(
            decisions,
            [
                (Case, 0, 1, 1),
                (EarlyReturn, 1, 0, 1),
                (Case, 0, 1, 0),
                (EarlyReturn, 1, 0, 1),
            ]
        );
    }
}

#[test]
fn throws_are_early_exits_and_catch_clauses_are_decisions() {
    for decisions in both("decide.parseCents") {
        assert_eq!(
            decisions,
            [
                (Branch, 0, 1, 1),
                (EarlyReturn, 1, 0, 1),
                (EarlyReturn, 0, 0, 1),
                (Catch, 0, 1, 1),
            ]
        );
    }
}

#[test]
fn every_symbol_has_an_entry_even_without_decisions() {
    for lang in ["javascript", "typescript"] {
        let root = fixture(lang);
        let symbols: Symbols = produce(lang, &root);
        let decisions: Decisions = produce(lang, &root);
        let keys: Vec<_> = symbols
            .symbols
            .iter()
            .map(cupel_core::syntax::SymbolDef::key)
            .collect();
        let entries: Vec<_> = decisions.symbols.iter().map(|e| e.symbol.clone()).collect();
        assert_eq!(entries, keys, "{lang}");
    }
}

#[test]
fn nullish_coalescing_and_optional_chaining_are_not_decisions_but_or_is() {
    let dir = tree(&[(
        "src/a.js",
        "function f(a, b) {\n  const c = a ?? b?.c;\n  return c || 0;\n}\n",
    )]);
    assert_eq!(
        decisions_of("javascript", dir.path(), "a.f"),
        [(BooleanOp, 0, 1, 1)]
    );
}

#[test]
fn every_loop_form_is_a_loop_and_named_closures_own_their_decisions() {
    let dir = tree(&[(
        "src/a.ts",
        "export function f(items: number[]): number {\n  let n = 0;\n  for (let i = 0; i < 3; i++) {}\n  for (const k in items) {}\n  while (n < 3) { n++; }\n  do { n--; } while (n > 0);\n  const g = (x: number) => (x > 0 ? x : -x);\n  items.forEach((x) => { if (x) { n += x; } });\n  return n;\n}\n",
    )]);
    assert_eq!(
        decisions_of("typescript", dir.path(), "a.f"),
        [
            (Loop, 0, 1, 1),
            (Loop, 0, 1, 1),
            (Loop, 0, 1, 1),
            (Loop, 0, 1, 1),
            (Branch, 0, 1, 1),
        ]
    );
    assert_eq!(
        decisions_of("typescript", dir.path(), "a.f.g"),
        [(Ternary, 0, 1, 1)]
    );
}

#[test]
fn test_callbacks_own_the_decisions_inside_them() {
    let dir = tree(&[(
        "src/a.test.tsx",
        "it(\"renders\", () => {\n  for (const row of rows()) {\n    expect(row).toBeTruthy();\n  }\n});\n",
    )]);
    assert_eq!(
        decisions_of("typescript", dir.path(), "a.renders"),
        [(Loop, 0, 1, 1)]
    );
}

#[test]
fn for_of_over_literal_arrays_is_a_table_and_other_loops_are_not() {
    let dir = tree(&[(
        "t.js",
        "export function cases() {\n\
         \x20 for (const x of [1, 2]) { f(x); }\n\
         \x20 const table = [[1, 2]];\n\
         \x20 for (const [a, b] of table) { f(a + b); }\n\
         \x20 for (const x of computed()) { f(x); }\n\
         \x20 for (let i = 0; i < 3; i++) { f(i); }\n\
         \x20 for (const k in { a: 1 }) { f(k); }\n\
         }\n",
    )]);
    assert_eq!(
        loop_tables("javascript", dir.path(), "t.cases"),
        [true, true, false, false, false]
    );
}
