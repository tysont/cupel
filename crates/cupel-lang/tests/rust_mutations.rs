// ABOUTME: The Rust MutationSites producer: operators, sets, replacement spellings, and site hashes;
// ABOUTME: properties: site hashes survive moves and layout, and every replacement still parses.
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
        summary(&sites_of("rust", &fixture("rust"), "pricing::discounted")),
        [
            row(ArithMulDiv, "*", &["/"], 2),
            row(ArithMulDiv, "/", &["*"], 2),
            row(Constant, "100", &["0"], 2),
            row(
                Negation,
                "member && cut != 0",
                &["!(member && cut != 0)"],
                3
            ),
            row(BooleanOp, "&&", &["||"], 3),
            row(Equality, "!=", &["=="], 3),
            row(ZeroOne, "0", &["1"], 3),
            row(ReturnValue, "price - cut", &["Default::default()"], 4),
            row(ArithAddSub, "-", &["+"], 4),
        ]
    );
}

#[test]
fn tally_has_literal_compound_assignment_and_statement_deletion_sites() {
    assert_eq!(
        summary(&sites_of("rust", &fixture("rust"), "pricing::tally")),
        [
            row(ZeroOne, "0", &["1"], 10),
            row(BooleanLiteral, "false", &["true"], 11),
            row(StatementDeletion, "total += entry;", &[""], 13),
            row(ArithAddSub, "+=", &["-="], 13),
            row(Negation, "total < -1", &["!(total < -1)"], 14),
            row(Relational, "<", &["<="], 14),
            row(ZeroOne, "1", &["0"], 14),
            row(StatementDeletion, "overdrawn = true;", &[""], 15),
            row(BooleanLiteral, "true", &["false"], 15),
            row(StatementDeletion, "log_total(total);", &[""], 18),
        ]
    );
}

#[test]
fn integer_literals_compared_against_have_boundary_sites() {
    let sites = sites_of("rust", &fixture("rust"), "decide::classify");
    let boundary: Vec<_> = summary(&sites)
        .into_iter()
        .filter(|(operator, ..)| *operator == Boundary)
        .collect();
    assert_eq!(
        boundary,
        [
            row(Boundary, "0", &["0 + 1", "0 - 1"], 2),
            row(Boundary, "0", &["0 + 1", "0 - 1"], 4),
        ]
    );
}

#[test]
fn site_hashes_are_sixteen_hex_digits_and_distinct_within_a_symbol() {
    let sites = sites_of("rust", &fixture("rust"), "pricing::tally");
    let mut hashes: Vec<&str> = sites.iter().map(|s| s.site_hash.as_str()).collect();
    assert!(hashes.iter().all(|h| h.len() == 16));
    hashes.sort_unstable();
    hashes.dedup();
    assert_eq!(hashes.len(), sites.len());
}

/// The grammar every rust fixture file parses with.
fn grammar(_: &str) -> tree_sitter::Language {
    tree_sitter_rust::LANGUAGE.into()
}

#[test]
fn every_replacement_in_the_fixture_repository_parses() {
    assert_every_replacement_parses("rust", &fixture("rust"), grammar);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// Moving the file and changing its layout moves sites to other lines but keeps their
    /// hashes; and every replacement still parses after the move.
    #[test]
    fn site_hashes_survive_moves_and_layout(
        indent in prop::sample::select(vec!["    ", "  ", "\t", "        "]),
        blank in prop::sample::select(vec!["", "\n"]),
        prefix in prop::sample::select(vec!["", "// moved\n\n", "fn earlier() {}\n\n"]),
        path in prop::sample::select(vec!["src/pricing.rs", "src/money/pricing.rs", "lib/p.rs"]),
    ) {
        let original = tree(&[(
            "src/pricing.rs",
            &std::fs::read_to_string(fixture("rust").join("src/pricing.rs")).unwrap(),
        )]);
        let moved = tree(&[(path, &format!("{prefix}{}", relaid("rust", "src/pricing.rs", "    ", indent, blank)))]);
        let mut expected = site_hashes("rust", original.path());
        let mut actual = site_hashes("rust", moved.path());
        actual.retain(|(name, _)| name != "earlier");
        expected.sort();
        actual.sort();
        prop_assert_eq!(actual, expected);
        assert_every_replacement_parses("rust", moved.path(), grammar);
    }
}
