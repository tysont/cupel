// ABOUTME: The Go Decisions producer: if/else-if chains, for and range loops, expression, type, and
// ABOUTME: select cases, short-circuit operators, early returns, and which symbol owns each decision.
mod support;

use cupel_core::syntax::{DecisionKind, Decisions, Symbols};
use support::{case_continuations, fixture, loop_tables, produce, tree};

use DecisionKind::{BooleanOp, Branch, Case, EarlyReturn, Loop};

/// (kind, nesting, cyclomatic, cognitive) for each decision of the symbol `display`.
fn decisions_of(root: &std::path::Path, display: &str) -> Vec<(DecisionKind, u32, u32, u32)> {
    let symbols: Symbols = produce("go", root);
    let decisions: Decisions = produce("go", root);
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

#[test]
fn an_else_if_chain_is_flat_and_conditions_do_not_nest() {
    assert_eq!(
        decisions_of(&fixture("go"), "ledger.classify"),
        [(Branch, 0, 1, 1), (Branch, 0, 1, 1), (BooleanOp, 0, 1, 1)]
    );
}

#[test]
fn a_return_inside_a_branch_inside_a_range_loop_is_nested_twice() {
    assert_eq!(
        decisions_of(&fixture("go"), "ledger.firstOverdraft"),
        [(Loop, 0, 1, 1), (Branch, 1, 1, 1), (EarlyReturn, 2, 0, 1)]
    );
}

#[test]
fn an_error_check_that_returns_is_a_branch_with_an_early_return() {
    assert_eq!(
        decisions_of(&fixture("go"), "ledger.parseCents"),
        [(Branch, 0, 1, 1), (EarlyReturn, 1, 0, 1)]
    );
}

#[test]
fn switch_and_type_switch_cases_count_once_each_and_default_not_at_all() {
    let expected = [
        (Case, 0, 1, 1),
        (EarlyReturn, 1, 0, 1),
        (Case, 0, 1, 0),
        (EarlyReturn, 1, 0, 1),
    ];
    assert_eq!(decisions_of(&fixture("go"), "ledger.describe"), expected);
    assert_eq!(decisions_of(&fixture("go"), "ledger.kindOf"), expected);
}

#[test]
fn select_cases_in_an_endless_loop_are_nested_cases() {
    assert_eq!(
        decisions_of(&fixture("go"), "ledger.drain"),
        [(Loop, 0, 1, 1), (Case, 1, 1, 1), (Case, 1, 1, 0)]
    );
}

#[test]
fn every_symbol_has_an_entry_even_without_decisions() {
    let root = fixture("go");
    let symbols: Symbols = produce("go", &root);
    let decisions: Decisions = produce("go", &root);
    let keys: Vec<_> = symbols
        .symbols
        .iter()
        .map(cupel_core::syntax::SymbolDef::key)
        .collect();
    let entries: Vec<_> = decisions.symbols.iter().map(|e| e.symbol.clone()).collect();
    assert_eq!(entries, keys);
}

#[test]
fn a_branch_inside_an_else_if_nests_once_and_a_final_return_is_not_early() {
    let dir = tree(&[(
        "f.go",
        "package f\n\nfunc f(x int) int {\n\tif x > 0 {\n\t\treturn 1\n\t} else if x < -10 || x == -5 {\n\t\tif x == -5 {\n\t\t\tx--\n\t\t}\n\t}\n\treturn 2\n\t// done\n}\n",
    )]);
    assert_eq!(
        decisions_of(dir.path(), "f.f"),
        [
            (Branch, 0, 1, 1),
            (EarlyReturn, 1, 0, 1),
            (Branch, 0, 1, 1),
            (BooleanOp, 0, 1, 1),
            (Branch, 1, 1, 1),
        ]
    );
}

#[test]
fn named_function_literals_own_their_decisions_and_inline_literals_do_not() {
    let dir = tree(&[(
        "f.go",
        "package f\n\nfunc outer(v []int) int {\n\tpositive := func(x int) bool { return x > 0 && x < 10 }\n\tn := 0\n\tgo func() {\n\t\tfor i := 0; i < len(v); i++ {\n\t\t}\n\t}()\n\tif positive(n) {\n\t\tn++\n\t}\n\treturn n\n}\n",
    )]);
    assert_eq!(
        decisions_of(dir.path(), "f.outer"),
        [(Loop, 0, 1, 1), (Branch, 0, 1, 1)]
    );
    assert_eq!(
        decisions_of(dir.path(), "f.outer.positive"),
        [(BooleanOp, 0, 1, 1)]
    );
}

#[test]
fn ranges_over_composite_literals_are_tables_and_others_are_not() {
    let dir = tree(&[
        ("go.mod", "module t\n"),
        (
            "t.go",
            "package t\n\nfunc Cases() {\n\
             \tfor _, x := range []int{1, 2} {\n\t\tf(x)\n\t}\n\
             \tcases := []struct{ in, want int }{{1, 2}}\n\
             \tfor _, c := range cases {\n\t\tf(c.in)\n\t}\n\
             \tfor i := 0; i < 3; i++ {\n\t\tf(i)\n\t}\n\
             \tfor _, x := range computed() {\n\t\tf(x)\n\t}\n\
             }\n\nfunc f(int) {}\n\nfunc computed() []int { return nil }\n",
        ),
    ]);
    assert_eq!(
        loop_tables("go", dir.path(), "t.Cases"),
        [true, true, false, false]
    );
}

#[test]
fn the_fixtures_table_driven_test_ranges_over_a_table() {
    assert_eq!(
        loop_tables("go", &fixture("go"), "ledger.TestClassify"),
        [true]
    );
}

#[test]
fn every_case_after_the_first_continues_its_multiway_decision() {
    let mut expected = vec![true; 12];
    expected[0] = false;
    assert_eq!(
        case_continuations("go", &fixture("go"), "ledger.Month"),
        expected
    );
}
