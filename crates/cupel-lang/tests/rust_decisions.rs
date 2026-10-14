// ABOUTME: The Rust Decisions producer: decision kinds, nesting (else-if chains are flat, conditions
// ABOUTME: do not nest), weights from the tables, early returns, and which symbol owns each decision.
mod support;

use cupel_core::syntax::{DecisionKind, Decisions, Symbols};
use support::{case_continuations, decisions_of, fixture, loop_tables, produce, tree};

use DecisionKind::{BooleanOp, Branch, Case, EarlyReturn, Loop};

#[test]
fn an_else_if_chain_is_flat_and_conditions_do_not_nest() {
    assert_eq!(
        decisions_of("rust", &fixture("rust"), "decide::classify"),
        [(Branch, 0, 1, 1), (Branch, 0, 1, 1), (BooleanOp, 0, 1, 1),]
    );
}

#[test]
fn a_return_inside_a_branch_inside_a_loop_is_nested_twice() {
    assert_eq!(
        decisions_of("rust", &fixture("rust"), "decide::first_overdraft"),
        [(Loop, 0, 1, 1), (Branch, 1, 1, 1), (EarlyReturn, 2, 0, 1)]
    );
}

#[test]
fn the_question_mark_operator_is_an_early_return() {
    assert_eq!(
        decisions_of("rust", &fixture("rust"), "decide::parse_cents"),
        [(EarlyReturn, 0, 0, 1)]
    );
}

#[test]
fn match_arms_are_cases_and_only_the_first_weighs_toward_cognitive() {
    assert_eq!(
        decisions_of("rust", &fixture("rust"), "decide::describe"),
        [(Case, 0, 1, 1), (Case, 0, 1, 0)]
    );
}

#[test]
fn every_symbol_has_an_entry_even_without_decisions() {
    let root = fixture("rust");
    let symbols: Symbols = produce("rust", &root);
    let decisions: Decisions = produce("rust", &root);
    let keys: Vec<_> = symbols
        .symbols
        .iter()
        .map(cupel_core::syntax::SymbolDef::key)
        .collect();
    let entries: Vec<_> = decisions.symbols.iter().map(|e| e.symbol.clone()).collect();
    assert_eq!(entries, keys);
}

#[test]
fn a_final_return_is_not_early_and_comments_do_not_change_that() {
    let dir = tree(&[(
        "src/lib.rs",
        "fn f(x: i32) -> i32 {\n    if x > 0 {\n        return 1;\n    }\n    return 2;\n    // done\n}\n",
    )]);
    assert_eq!(
        decisions_of("rust", dir.path(), "f"),
        [(Branch, 0, 1, 1), (EarlyReturn, 1, 0, 1)]
    );
}

#[test]
fn closures_and_nested_functions_own_their_decisions_and_inline_closures_do_not() {
    let dir = tree(&[(
        "src/lib.rs",
        "fn outer(v: &[i32]) -> usize {\n    \
         let positive = |x: &i32| if *x > 0 { 1 } else { 0 };\n    \
         fn inner(x: i32) -> bool { x > 0 || x < -10 }\n    \
         v.iter().filter(|x| **x > 0 && inner(**x)).map(positive).sum()\n}\n",
    )]);
    assert_eq!(
        decisions_of("rust", dir.path(), "outer"),
        [(BooleanOp, 0, 1, 1)]
    );
    assert_eq!(
        decisions_of("rust", dir.path(), "outer::positive"),
        [(Branch, 0, 1, 1)]
    );
    assert_eq!(
        decisions_of("rust", dir.path(), "outer::inner"),
        [(BooleanOp, 0, 1, 1)]
    );
}

#[test]
fn while_loop_let_else_and_wildcard_arms() {
    let dir = tree(&[(
        "src/lib.rs",
        "fn f(o: Option<i32>) -> i32 {\n    \
         let Some(mut n) = o else { return 0 };\n    \
         while n > 10 { n -= 1; }\n    \
         loop { match n { 1 => break, _ => n -= 1 } }\n    \
         n\n}\n",
    )]);
    assert_eq!(
        decisions_of("rust", dir.path(), "f"),
        [
            (Branch, 0, 1, 1),
            (EarlyReturn, 1, 0, 1),
            (Loop, 0, 1, 1),
            (Loop, 0, 1, 1),
            (Case, 1, 1, 1),
        ]
    );
}

#[test]
fn loops_over_literal_arrays_vectors_slices_and_constants_are_tables_and_others_are_not() {
    let dir = tree(&[
        ("Cargo.toml", "[package]\nname = \"t\"\n"),
        (
            "src/lib.rs",
            "pub fn cases() {\n\
             \x20   for x in [1, 2] { f(x); }\n\
             \x20   for x in &[1, 2] { f(*x); }\n\
             \x20   for x in vec![1] { f(x); }\n\
             \x20   for x in [1, 2].iter() { f(*x); }\n\
             \x20   let table = [(1, 2)];\n\
             \x20   for (a, b) in table { f(a + b); }\n\
             \x20   let more = vec![3];\n\
             \x20   for x in &more { f(*x); }\n\
             \x20   for x in 0..3 { f(x); }\n\
             \x20   for x in computed() { f(x); }\n\
             \x20   while g() { f(1); }\n\
             \x20   let late = [1];\n\
             \x20   for x in late.iter() { f(*x) }\n\
             \x20   for x in CASES { f(x); }\n\
             \x20   for x in Kind::ALL.iter() { f(x); }\n\
             \x20   for x in &schema::PUBLISHED { f(x); }\n\
             \x20   for x in Kind::all() { f(x); }\n\
             }\n",
        ),
    ]);
    assert_eq!(
        loop_tables("rust", dir.path(), "cases"),
        [
            true, true, true, true, true, true, false, false, false, true, true, true, true, false
        ]
    );
}

#[test]
fn every_case_after_the_first_continues_its_multiway_decision() {
    let mut expected = vec![true; 12];
    expected[0] = false;
    assert_eq!(
        case_continuations("rust", &fixture("rust"), "complexity::month"),
        expected
    );
}
