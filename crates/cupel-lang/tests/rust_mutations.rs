// ABOUTME: The Rust MutationSites producer: operators, sets, replacement spellings, and site hashes;
// ABOUTME: properties: site hashes survive moves and layout, and every replacement still parses.
mod support;

use std::path::Path;

use cupel_core::syntax::{MutationOperator, MutationSites, OperatorSet, Site, Symbols};
use proptest::prelude::*;
use support::{byte_range, fixture, produce, tree};

use MutationOperator::{
    ArithAddSub, ArithMulDiv, BooleanLiteral, BooleanOp, Boundary, Constant, Equality, Negation,
    Relational, ReturnValue, StatementDeletion, ZeroOne,
};

fn sites_of(root: &Path, display: &str) -> Vec<Site> {
    let symbols: Symbols = produce("rust", root);
    let sites: MutationSites = produce("rust", root);
    let key = symbols
        .symbols
        .iter()
        .find(|s| s.display == display)
        .unwrap_or_else(|| panic!("no symbol {display}"))
        .key();
    sites
        .symbols
        .into_iter()
        .find(|entry| entry.symbol == key)
        .unwrap_or_else(|| panic!("no sites entry for {display}"))
        .sites
}

/// (operator, set, from, to, line) per site.
fn summary(sites: &[Site]) -> Vec<(MutationOperator, OperatorSet, String, Vec<String>, u32)> {
    sites
        .iter()
        .map(|s| (s.operator, s.set, s.from.clone(), s.to.clone(), s.span.line))
        .collect()
}

fn row(
    operator: MutationOperator,
    from: &str,
    to: &[&str],
    line: u32,
) -> (MutationOperator, OperatorSet, String, Vec<String>, u32) {
    let set = match operator {
        Relational | Equality | ArithAddSub | ArithMulDiv | BooleanOp | BooleanLiteral
        | ZeroOne => OperatorSet::Default,
        _ => OperatorSet::Extended,
    };
    (
        operator,
        set,
        from.to_owned(),
        to.iter().map(|t| (*t).to_owned()).collect(),
        line,
    )
}

#[test]
fn discounted_has_arithmetic_boolean_equality_and_extended_sites() {
    assert_eq!(
        summary(&sites_of(&fixture("rust"), "pricing::discounted")),
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
        summary(&sites_of(&fixture("rust"), "pricing::tally")),
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
    let sites = sites_of(&fixture("rust"), "decide::classify");
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
    let sites = sites_of(&fixture("rust"), "pricing::tally");
    let mut hashes: Vec<&str> = sites.iter().map(|s| s.site_hash.as_str()).collect();
    assert!(hashes.iter().all(|h| h.len() == 16));
    hashes.sort_unstable();
    hashes.dedup();
    assert_eq!(hashes.len(), sites.len());
}

/// Every replacement of every site in `root`'s Rust files leaves a file that parses cleanly.
fn every_replacement_parses(root: &Path) {
    let sites: MutationSites = produce("rust", root);
    let symbols: Symbols = produce("rust", root);
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();
    let mut checked = 0;
    for entry in &sites.symbols {
        let file = &symbols
            .symbols
            .iter()
            .find(|s| s.key() == entry.symbol)
            .unwrap()
            .file;
        let source = std::fs::read_to_string(root.join(file)).unwrap();
        for site in &entry.sites {
            let (start, end) = byte_range(&source, site);
            assert_eq!(&source[start..end], site.from, "{file}");
            for to in &site.to {
                let mutated = format!("{}{to}{}", &source[..start], &source[end..]);
                let tree = parser.parse(&mutated, None).unwrap();
                assert!(
                    !tree.root_node().has_error(),
                    "{file}:{}: {} -> {to} does not parse",
                    site.span.line,
                    site.from
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0);
}

#[test]
fn every_replacement_in_the_fixture_repository_parses() {
    every_replacement_parses(&fixture("rust"));
}

/// The fixture's `pricing.rs`, reformatted: indentation, blank lines, and line breaks change.
fn relaid(indent: &str, blank: &str) -> String {
    std::fs::read_to_string(fixture("rust").join("src/pricing.rs"))
        .unwrap()
        .lines()
        .map(|line| {
            let depth = line.len() - line.trim_start().len();
            format!("{}{}", indent.repeat(depth / 4), line.trim_start())
        })
        .collect::<Vec<_>>()
        .join(&format!("\n{blank}"))
}

fn site_hashes(root: &Path) -> Vec<(String, Vec<String>)> {
    let symbols: Symbols = produce("rust", root);
    let sites: MutationSites = produce("rust", root);
    sites
        .symbols
        .iter()
        .map(|entry| {
            let name = symbols
                .symbols
                .iter()
                .find(|s| s.key() == entry.symbol)
                .unwrap()
                .name
                .clone();
            (
                name,
                entry
                    .sites
                    .iter()
                    .map(|s| s.site_hash.to_string())
                    .collect(),
            )
        })
        .collect()
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
        let moved = tree(&[(path, &format!("{prefix}{}", relaid(indent, blank)))]);
        let mut expected = site_hashes(original.path());
        let mut actual = site_hashes(moved.path());
        actual.retain(|(name, _)| name != "earlier");
        expected.sort();
        actual.sort();
        prop_assert_eq!(actual, expected);
        every_replacement_parses(moved.path());
    }
}
