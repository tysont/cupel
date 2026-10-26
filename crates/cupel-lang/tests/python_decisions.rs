// ABOUTME: The Python Decisions producer: if/elif chains, loops and comprehensions, `and`/`or`, ternaries,
// ABOUTME: `match` cases, `except` handlers, and early `return`/`raise`, with nesting and weights.
mod support;

use cupel_core::syntax::{DecisionKind, Decisions, Symbols};
use support::{
    case_continuations, decisions_of, expression_clauses, fixture, loop_tables, produce, tree,
};

use DecisionKind::{BooleanOp, Branch, Case, Catch, EarlyReturn, Loop, Ternary};

#[test]
fn an_elif_chain_is_flat_and_conditions_do_not_nest() {
    assert_eq!(
        decisions_of("python", &fixture("python"), "ledger.decide.classify"),
        [(Branch, 0, 1, 1), (Branch, 0, 1, 1), (BooleanOp, 0, 1, 1)]
    );
}

#[test]
fn a_return_inside_a_branch_inside_a_loop_is_nested_twice() {
    assert_eq!(
        decisions_of(
            "python",
            &fixture("python"),
            "ledger.decide.first_overdraft"
        ),
        [(Loop, 0, 1, 1), (Branch, 1, 1, 1), (EarlyReturn, 2, 0, 1)]
    );
}

#[test]
fn an_except_handler_is_a_catch_and_nests_its_early_return() {
    assert_eq!(
        decisions_of("python", &fixture("python"), "ledger.decide.parse_cents"),
        [(Catch, 0, 1, 1), (EarlyReturn, 1, 0, 1)]
    );
}

#[test]
fn a_raise_that_is_not_final_is_an_early_return() {
    assert_eq!(
        decisions_of(
            "python",
            &fixture("python"),
            "ledger.decide.require_positive"
        ),
        [(Branch, 0, 1, 1), (EarlyReturn, 1, 0, 1)]
    );
}

#[test]
fn match_cases_skip_the_wildcard_and_only_the_first_weighs_toward_cognitive() {
    assert_eq!(
        decisions_of("python", &fixture("python"), "ledger.decide.describe"),
        [
            (Case, 0, 1, 1),
            (EarlyReturn, 1, 0, 1),
            (Case, 0, 1, 0),
            (EarlyReturn, 1, 0, 1),
        ]
    );
}

#[test]
fn conditional_expressions_and_comprehension_clauses() {
    let root = fixture("python");
    assert_eq!(
        decisions_of("python", &root, "ledger.decide.sign"),
        [(Ternary, 0, 1, 1)]
    );
    assert_eq!(
        decisions_of("python", &root, "ledger.decide.positives"),
        [(Loop, 0, 1, 1), (Branch, 0, 1, 1)]
    );
}

#[test]
fn every_symbol_has_an_entry_even_without_decisions() {
    let root = fixture("python");
    let symbols: Symbols = produce("python", &root);
    let decisions: Decisions = produce("python", &root);
    let keys: Vec<_> = symbols
        .symbols
        .iter()
        .map(cupel_core::syntax::SymbolDef::key)
        .collect();
    let entries: Vec<_> = decisions.symbols.iter().map(|e| e.symbol.clone()).collect();
    assert_eq!(entries, keys);
}

#[test]
fn a_raise_ending_the_final_else_is_final_and_comments_do_not_change_that() {
    let dir = tree(&[(
        "m.py",
        "def f(x):\n    if x:\n        return 1\n    else:\n        raise ValueError(x)\n    # done\n",
    )]);
    assert_eq!(
        decisions_of("python", dir.path(), "m.f"),
        [(Branch, 0, 1, 1), (EarlyReturn, 1, 0, 1)]
    );
}

#[test]
fn nested_functions_and_lambdas_own_their_decisions_and_inline_lambdas_do_not() {
    let dir = tree(&[(
        "m.py",
        "def outer(values):\n    \
         positive = lambda x: 1 if x > 0 else 0\n    \
         def inner(x):\n        return x > 0 or x < -10\n    \
         return sorted(values, key=lambda v: v and inner(v))\n",
    )]);
    assert_eq!(
        decisions_of("python", dir.path(), "m.outer"),
        [(BooleanOp, 0, 1, 1)]
    );
    assert_eq!(
        decisions_of("python", dir.path(), "m.outer.positive"),
        [(Ternary, 0, 1, 1)]
    );
    assert_eq!(
        decisions_of("python", dir.path(), "m.outer.inner"),
        [(BooleanOp, 0, 1, 1)]
    );
}

#[test]
fn while_loops_exception_groups_guards_and_branches_inside_elif() {
    let dir = tree(&[(
        "m.py",
        "def f(n, kind):\n    \
         while n > 10:\n        n -= 1\n    \
         try:\n        g(n)\n    except* OSError:\n        pass\n    \
         match kind:\n        case int(k) if k > 0:\n            pass\n    \
         if n:\n        pass\n    elif kind:\n        if n < 0:\n            pass\n    \
         return n\n",
    )]);
    assert_eq!(
        decisions_of("python", dir.path(), "m.f"),
        [
            (Loop, 0, 1, 1),
            (Catch, 0, 1, 1),
            (Case, 0, 1, 1),
            (Branch, 1, 1, 1),
            (Branch, 0, 1, 1),
            (Branch, 0, 1, 1),
            (Branch, 1, 1, 1),
        ]
    );
}

#[test]
fn loops_over_literal_lists_and_tuples_are_tables_and_others_are_not() {
    let dir = tree(&[(
        "t.py",
        "def cases():\n\
         \x20   for x in [1, 2]:\n        f(x)\n\
         \x20   for x in (1, 2):\n        f(x)\n\
         \x20   table = [(1, 2)]\n\
         \x20   for a, b in table:\n        f(a + b)\n\
         \x20   for x in range(3):\n        f(x)\n\
         \x20   for x in computed():\n        f(x)\n",
    )]);
    assert_eq!(
        loop_tables("python", dir.path(), "t.cases"),
        [true, true, true, false, false]
    );
}

/// (kind, expression) of each decision of the function `display` of a file `t.py` holding `source`.
fn clauses_of(source: &str, display: &str) -> Vec<(DecisionKind, bool)> {
    let dir = tree(&[("t.py", source)]);
    expression_clauses("python", dir.path(), display)
}

#[test]
fn comprehension_clauses_are_expression_decisions_and_statements_are_not() {
    assert_eq!(
        clauses_of(
            "def mixed(xs):\n\
             \x20   for x in xs:\n        if x:\n            f(x)\n\
             \x20   return [y for y in xs if y], {k: v for k, v in xs}, any(z for z in xs)\n",
            "t.mixed"
        ),
        [
            (Loop, false),
            (Branch, false),
            (Loop, true),
            (Branch, true),
            (Loop, true),
            (Loop, true),
        ]
    );
}

#[test]
fn every_case_after_the_first_continues_its_multiway_decision() {
    let mut expected = vec![true; 12];
    expected[0] = false;
    assert_eq!(
        case_continuations("python", &fixture("python"), "ledger.complexity.month"),
        expected
    );
}

#[test]
fn decisions_inside_a_lambda_are_expression_decisions() {
    // A lambda handed to the code under test computes values when that code calls it; its
    // decisions are not the enclosing function's control flow.
    assert_eq!(
        clauses_of(
            "def passes(f, g, h, t):\n\
             \x20   f(lambda x: 1 if x else 2)\n\
             \x20   g(key=lambda y: (3 if y else 4))\n\
             \x20   h(lambda z: [w for w in z if w and z])\n\
             \x20   return 5 if t else 6\n",
            "t.passes"
        ),
        [
            (Ternary, true),
            (Ternary, true),
            (Loop, true),
            (Branch, true),
            (BooleanOp, true),
            (Ternary, false),
        ]
    );
}
