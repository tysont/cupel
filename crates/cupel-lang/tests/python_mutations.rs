// ABOUTME: The Python MutationSites producer: operators, sets, Python spellings (`and`/`or`, `True`/`False`,
// ABOUTME: `is`/`is not`, `pass`, `None`); properties: hashes survive moves and relayout, replacements parse.
mod support;

use cupel_core::syntax::MutationOperator;
use proptest::prelude::*;
use support::{
    assert_every_replacement_parses, fixture, relaid, site_hashes, site_row as row, sites_of,
    summary, tree,
};

use MutationOperator::{
    ArithAddSub, ArithMulDiv, BooleanLiteral, BooleanOp, Boundary, Constant, Equality, Negation,
    Relational, ReturnValue, StatementDeletion, ZeroOne,
};

#[test]
fn discounted_has_arithmetic_boolean_equality_and_extended_sites() {
    assert_eq!(
        summary(&sites_of(
            "python",
            &fixture("python"),
            "ledger.pricing.discounted"
        )),
        [
            row(StatementDeletion, "cut = price * 15 / 100", &["pass"], 2),
            row(ArithMulDiv, "*", &["/"], 2),
            row(Constant, "15", &["0"], 2),
            row(ArithMulDiv, "/", &["*"], 2),
            row(Constant, "100", &["0"], 2),
            row(
                Negation,
                "member and cut != 0",
                &["not (member and cut != 0)"],
                3
            ),
            row(BooleanOp, "and", &["or"], 3),
            row(Equality, "!=", &["=="], 3),
            row(ZeroOne, "0", &["1"], 3),
            row(ReturnValue, "price - cut", &["None"], 4),
            row(ArithAddSub, "-", &["+"], 4),
            row(ReturnValue, "price", &["None"], 5),
        ]
    );
}

#[test]
fn tally_has_literal_augmented_assignment_and_statement_deletion_sites() {
    assert_eq!(
        summary(&sites_of(
            "python",
            &fixture("python"),
            "ledger.pricing.tally"
        )),
        [
            row(StatementDeletion, "total = 0", &["pass"], 9),
            row(ZeroOne, "0", &["1"], 9),
            row(StatementDeletion, "overdrawn = False", &["pass"], 10),
            row(BooleanLiteral, "False", &["True"], 10),
            row(StatementDeletion, "total += entry", &["pass"], 12),
            row(ArithAddSub, "+=", &["-="], 12),
            row(Negation, "total < -1", &["not (total < -1)"], 13),
            row(Relational, "<", &["<="], 13),
            row(ZeroOne, "1", &["0"], 13),
            row(StatementDeletion, "overdrawn = True", &["pass"], 14),
            row(BooleanLiteral, "True", &["False"], 14),
            row(StatementDeletion, "log_total(total)", &["pass"], 15),
            row(ReturnValue, "total, overdrawn", &["None"], 16),
        ]
    );
}

#[test]
fn identity_comparisons_are_equality_sites() {
    assert_eq!(
        summary(&sites_of(
            "python",
            &fixture("python"),
            "ledger.pricing.has_price"
        )),
        [
            row(ReturnValue, "item.price is not None", &["None"], 20),
            row(Equality, "is not", &["is"], 20),
        ]
    );
}

#[test]
fn integer_literals_compared_against_have_boundary_sites() {
    let sites = sites_of("python", &fixture("python"), "ledger.decide.classify");
    let boundary: Vec<_> = summary(&sites)
        .into_iter()
        .filter(|(operator, ..)| *operator == Boundary)
        .collect();
    assert_eq!(boundary, [row(Boundary, "0", &["0 + 1", "0 - 1"], 2)]);
}

#[test]
fn floor_division_elif_and_while_conditions_awaits_and_bare_returns() {
    let dir = tree(&[(
        "m.py",
        "async def f(a, b):\n    \
         while a // b >= 2:\n        a //= 2\n    \
         if a is None:\n        return\n    \
         elif a:\n        await g(a)\n    \
         return a\n",
    )]);
    assert_eq!(
        summary(&sites_of("python", dir.path(), "m.f")),
        [
            row(Negation, "a // b >= 2", &["not (a // b >= 2)"], 2),
            row(ArithMulDiv, "//", &["*"], 2),
            row(Relational, ">=", &[">"], 2),
            row(Constant, "2", &["0"], 2),
            row(Boundary, "2", &["2 + 1", "2 - 1"], 2),
            row(StatementDeletion, "a //= 2", &["pass"], 3),
            row(ArithMulDiv, "//=", &["*="], 3),
            row(Constant, "2", &["0"], 3),
            row(Negation, "a is None", &["not (a is None)"], 4),
            row(Equality, "is", &["is not"], 4),
            row(Negation, "a", &["not (a)"], 6),
            row(StatementDeletion, "await g(a)", &["pass"], 7),
            row(ReturnValue, "a", &["None"], 8),
        ]
    );
}

#[test]
fn site_hashes_are_sixteen_hex_digits_and_distinct_within_a_symbol() {
    let sites = sites_of("python", &fixture("python"), "ledger.pricing.tally");
    let mut hashes: Vec<&str> = sites.iter().map(|s| s.site_hash.as_str()).collect();
    assert!(hashes.iter().all(|h| h.len() == 16));
    hashes.sort_unstable();
    hashes.dedup();
    assert_eq!(hashes.len(), sites.len());
}

/// The grammar every python fixture file parses with.
fn grammar(_: &str) -> tree_sitter::Language {
    tree_sitter_python::LANGUAGE.into()
}

#[test]
fn every_replacement_in_the_fixture_repository_parses() {
    assert_every_replacement_parses("python", &fixture("python"), grammar);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// Moving the file and changing its indentation unit and blank lines moves sites to other
    /// lines but keeps their hashes; and every replacement still parses after the move.
    #[test]
    fn site_hashes_survive_moves_and_relayout(
        indent in prop::sample::select(vec!["    ", "  ", "\t", "        "]),
        blank in prop::sample::select(vec!["", "\n"]),
        prefix in prop::sample::select(vec!["", "# moved\n\n", "def earlier():\n    pass\n\n\n"]),
        path in prop::sample::select(vec!["src/ledger/pricing.py", "lib/money/pricing.py", "p.py"]),
    ) {
        let original = tree(&[(
            "src/ledger/pricing.py",
            &std::fs::read_to_string(fixture("python").join("src/ledger/pricing.py")).unwrap(),
        )]);
        let moved = tree(&[(path, &format!("{prefix}{}\n", relaid("python", "src/ledger/pricing.py", "    ", indent, blank)))]);
        let mut expected = site_hashes("python", original.path());
        let mut actual = site_hashes("python", moved.path());
        actual.retain(|(name, _)| name != "earlier");
        expected.sort();
        actual.sort();
        prop_assert_eq!(actual, expected);
        assert_every_replacement_parses("python", moved.path(), grammar);
    }
}
