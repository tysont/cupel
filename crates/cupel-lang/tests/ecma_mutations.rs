// ABOUTME: The JavaScript and TypeScript MutationSites producers: operators, sets, replacements, and site
// ABOUTME: hashes; properties: hashes survive moves and layout, every replacement re-parses (TSX too).
mod support;

use std::path::Path;

use cupel_core::syntax::MutationOperator;
use proptest::prelude::*;
use support::{
    SiteRow as Row, assert_every_replacement_parses, fixture, relaid, site_hashes, site_row as row,
    sites_of, summary, tree,
};

use MutationOperator::{
    ArithAddSub, ArithMulDiv, BooleanLiteral, BooleanOp, Boundary, Constant, Equality, Negation,
    Relational, ReturnValue, StatementDeletion, ZeroOne,
};

/// Asserts that `display` has `expected` sites in both fixture repositories: the TypeScript
/// `pricing.ts` is the JavaScript `pricing.js` typed.
fn assert_both(display: &str, expected: &[Row]) {
    let both =
        ["javascript", "typescript"].map(|lang| summary(&sites_of(lang, &fixture(lang), display)));
    assert_eq!(both, [expected.to_vec(), expected.to_vec()], "{display}");
}

#[test]
fn discounted_has_arithmetic_boolean_strict_equality_and_extended_sites() {
    assert_both(
        "pricing.discounted",
        &[
            row(ArithMulDiv, "*", &["/"], 2),
            row(Constant, "15", &["0"], 2),
            row(ArithMulDiv, "/", &["*"], 2),
            row(Constant, "100", &["0"], 2),
            row(
                Negation,
                "member && cut !== 0",
                &["!(member && cut !== 0)"],
                3,
            ),
            row(BooleanOp, "&&", &["||"], 3),
            row(Equality, "!==", &["==="], 3),
            row(ZeroOne, "0", &["1"], 3),
            row(ReturnValue, "price - cut", &["undefined"], 4),
            row(ArithAddSub, "-", &["+"], 4),
            row(ReturnValue, "price", &["undefined"], 6),
        ],
    );
}

#[test]
fn tally_has_literal_compound_assignment_and_statement_deletion_sites() {
    assert_both(
        "pricing.tally",
        &[
            row(ZeroOne, "0", &["1"], 10),
            row(BooleanLiteral, "false", &["true"], 11),
            row(StatementDeletion, "total += entry;", &[""], 13),
            row(ArithAddSub, "+=", &["-="], 13),
            row(Negation, "total < -1", &["!(total < -1)"], 14),
            row(Relational, "<", &["<="], 14),
            row(ZeroOne, "1", &["0"], 14),
            row(StatementDeletion, "overdrawn = true;", &[""], 15),
            row(BooleanLiteral, "true", &["false"], 15),
            row(StatementDeletion, "logTotal(total);", &[""], 18),
            row(ReturnValue, "{ total, overdrawn }", &["undefined"], 19),
        ],
    );
}

#[test]
fn loose_equality_is_negated_loosely() {
    assert_both(
        "pricing.logTotal",
        &[
            row(
                StatementDeletion,
                "console.log(total == null ? \"none\" : total);",
                &[""],
                23,
            ),
            row(Equality, "==", &["!="], 23),
        ],
    );
}

#[test]
fn integer_literals_compared_against_have_boundary_sites() {
    for lang in ["javascript", "typescript"] {
        let sites = sites_of(lang, &fixture(lang), "decide.classify");
        let boundary: Vec<_> = summary(&sites)
            .into_iter()
            .filter(|(operator, ..)| *operator == Boundary)
            .collect();
        let line = if lang == "javascript" { 2 } else { 6 };
        assert_eq!(
            boundary,
            [
                row(Boundary, "0", &["0 + 1", "0 - 1"], line),
                row(Boundary, "0", &["0 + 1", "0 - 1"], line),
            ],
            "{lang}"
        );
    }
}

#[test]
fn statements_outside_blocks_and_fractional_literals_are_not_sites() {
    let dir = tree(&[(
        "src/a.js",
        "function f(a) {\n  if (a) log(a);\n  return a < 0.5;\n}\n",
    )]);
    assert_eq!(
        summary(&sites_of("javascript", dir.path(), "a.f")),
        [
            row(Negation, "a", &["!(a)"], 2),
            row(ReturnValue, "a < 0.5", &["undefined"], 3),
            row(Relational, "<", &["<="], 3),
        ]
    );
}

#[test]
fn site_hashes_are_sixteen_hex_digits_and_distinct_within_a_symbol() {
    let sites = sites_of("javascript", &fixture("javascript"), "pricing.tally");
    let mut hashes: Vec<&str> = sites.iter().map(|s| s.site_hash.as_str()).collect();
    assert!(hashes.iter().all(|h| h.len() == 16));
    hashes.sort_unstable();
    hashes.dedup();
    assert_eq!(hashes.len(), sites.len());
}

/// The grammar a file of the fixture corpus parses with.
fn grammar(file: &str) -> tree_sitter::Language {
    match Path::new(file).extension().and_then(|e| e.to_str()) {
        Some("ts" | "mts" | "cts") => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Some("tsx") => tree_sitter_typescript::LANGUAGE_TSX.into(),
        _ => tree_sitter_javascript::LANGUAGE.into(),
    }
}

#[test]
fn every_replacement_in_the_fixture_repositories_parses_in_its_own_grammar() {
    assert_every_replacement_parses("javascript", &fixture("javascript"), grammar);
    // .ts and .tsx files each parse with their own grammar.
    assert!(assert_every_replacement_parses("typescript", &fixture("typescript"), grammar) >= 2);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// Moving a file and changing its layout moves sites to other lines but keeps their
    /// hashes; and every replacement still parses after the move.
    #[test]
    fn site_hashes_survive_moves_and_layout(
        (lang, file) in prop::sample::select(vec![
            ("javascript", "src/pricing.js"),
            ("typescript", "src/pricing.ts"),
            ("typescript", "src/widget.tsx"),
        ]),
        indent in prop::sample::select(vec!["  ", "    ", "\t", " "]),
        blank in prop::sample::select(vec!["", "\n"]),
        prefix in prop::sample::select(vec!["", "// moved\n\n", "function earlier() {}\n\n"]),
        dir in prop::sample::select(vec!["src", "src/money", "packages/app/lib"]),
    ) {
        let name = Path::new(file).file_name().unwrap().to_str().unwrap();
        let original = tree(&[(
            file,
            &std::fs::read_to_string(fixture(lang).join(file)).unwrap(),
        )]);
        let moved_path = format!("{dir}/{name}");
        let moved = tree(&[(
            moved_path.as_str(),
            &format!("{prefix}{}", relaid(lang, file, "  ", indent, blank)),
        )]);
        let mut expected = site_hashes(lang, original.path());
        let mut actual = site_hashes(lang, moved.path());
        actual.retain(|(name, _)| name != "earlier");
        expected.sort();
        actual.sort();
        prop_assert_eq!(actual, expected);
        assert_every_replacement_parses(lang, moved.path(), grammar);
    }
}
