// ABOUTME: The Go MutationSites producer: operators, sets, replacement spellings, and site hashes;
// ABOUTME: properties: site hashes survive moves and layout, and every replacement still parses.
mod support;

use std::path::Path;

use cupel_core::syntax::{MutationOperator, MutationSites, OperatorSet, Site, Symbols};
use proptest::prelude::*;
use support::{fixture, produce, tree};

use MutationOperator::{
    ArithAddSub, ArithMulDiv, BooleanLiteral, BooleanOp, Boundary, Constant, Equality, Negation,
    Relational, ReturnValue, StatementDeletion, ZeroOne,
};

fn sites_of(root: &Path, display: &str) -> Vec<Site> {
    let symbols: Symbols = produce("go", root);
    let sites: MutationSites = produce("go", root);
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

type SiteRow = (MutationOperator, OperatorSet, String, Vec<String>, u32);

/// (operator, set, from, to, line) per site.
fn summary(sites: &[Site]) -> Vec<SiteRow> {
    sites
        .iter()
        .map(|s| (s.operator, s.set, s.from.clone(), s.to.clone(), s.span.line))
        .collect()
}

fn row(operator: MutationOperator, from: &str, to: &[&str], line: u32) -> SiteRow {
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
        summary(&sites_of(&fixture("go"), "ledger.discounted")),
        [
            row(ArithMulDiv, "*", &["/"], 6),
            row(ArithMulDiv, "/", &["*"], 6),
            row(Constant, "100", &["0"], 6),
            row(
                Negation,
                "member && cut != 0",
                &["!(member && cut != 0)"],
                7
            ),
            row(BooleanOp, "&&", &["||"], 7),
            row(Equality, "!=", &["=="], 7),
            row(ZeroOne, "0", &["1"], 7),
            row(ArithAddSub, "-", &["+"], 8),
            row(ReturnValue, "price", &["nil"], 10),
        ]
    );
}

#[test]
fn tally_has_literal_compound_assignment_and_statement_deletion_sites() {
    assert_eq!(
        summary(&sites_of(&fixture("go"), "ledger.tally")),
        [
            row(ZeroOne, "0", &["1"], 14),
            row(BooleanLiteral, "false", &["true"], 15),
            row(StatementDeletion, "total += entry", &[""], 17),
            row(ArithAddSub, "+=", &["-="], 17),
            row(Negation, "total < -1", &["!(total < -1)"], 18),
            row(Relational, "<", &["<="], 18),
            row(ZeroOne, "1", &["0"], 18),
            row(StatementDeletion, "overdrawn = true", &[""], 19),
            row(BooleanLiteral, "true", &["false"], 19),
            row(StatementDeletion, "logTotal(total)", &[""], 22),
            row(ReturnValue, "total", &["nil"], 23),
            row(ReturnValue, "overdrawn", &["nil"], 23),
        ]
    );
}

#[test]
fn integer_literals_compared_against_have_boundary_sites() {
    let boundary: Vec<_> = summary(&sites_of(&fixture("go"), "ledger.classify"))
        .into_iter()
        .filter(|(operator, ..)| *operator == Boundary)
        .collect();
    assert_eq!(
        boundary,
        [
            row(Boundary, "0", &["0 + 1", "0 - 1"], 14),
            row(Boundary, "0", &["0 + 1", "0 - 1"], 16),
        ]
    );
}

#[test]
fn increments_loop_conditions_and_error_returns() {
    let dir = tree(&[(
        "f.go",
        "package f\n\nfunc f(n int) (int, error) {\n\tfor n > 1 {\n\t\tn--\n\t}\n\tfor i := 0; i < 3; i++ {\n\t\tn *= 2\n\t}\n\treturn n, err()\n}\n",
    )]);
    assert_eq!(
        summary(&sites_of(dir.path(), "f.f")),
        [
            row(Negation, "n > 1", &["!(n > 1)"], 4),
            row(Relational, ">", &[">="], 4),
            row(ZeroOne, "1", &["0"], 4),
            row(Boundary, "1", &["1 + 1", "1 - 1"], 4),
            row(StatementDeletion, "n--", &[""], 5),
            row(ArithAddSub, "--", &["++"], 5),
            row(ZeroOne, "0", &["1"], 7),
            row(Negation, "i < 3", &["!(i < 3)"], 7),
            row(Relational, "<", &["<="], 7),
            row(Constant, "3", &["0"], 7),
            row(Boundary, "3", &["3 + 1", "3 - 1"], 7),
            row(ArithAddSub, "++", &["--"], 7),
            row(StatementDeletion, "n *= 2", &[""], 8),
            row(ArithMulDiv, "*=", &["/="], 8),
            row(Constant, "2", &["0"], 8),
            row(ReturnValue, "n", &["nil"], 10),
            row(ReturnValue, "err()", &["nil"], 10),
        ]
    );
}

#[test]
fn site_hashes_are_sixteen_hex_digits_and_distinct_within_a_symbol() {
    let sites = sites_of(&fixture("go"), "ledger.tally");
    let mut hashes: Vec<&str> = sites.iter().map(|s| s.site_hash.as_str()).collect();
    assert!(hashes.iter().all(|h| h.len() == 16));
    hashes.sort_unstable();
    hashes.dedup();
    assert_eq!(hashes.len(), sites.len());
}

/// Every replacement of every site in `root`'s Go files leaves a file that parses cleanly.
fn every_replacement_parses(root: &Path) {
    let sites: MutationSites = produce("go", root);
    let symbols: Symbols = produce("go", root);
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
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

/// The byte range of a site from its 1-based line and byte columns.
fn byte_range(source: &str, site: &Site) -> (usize, usize) {
    let offset = |line: u32, col: u32| {
        let start: usize = source
            .split_inclusive('\n')
            .take(line as usize - 1)
            .map(str::len)
            .sum();
        start + col as usize - 1
    };
    (
        offset(site.span.line, site.span.col),
        offset(site.span.end_line, site.span.end_col),
    )
}

#[test]
fn every_replacement_in_the_fixture_repository_parses() {
    every_replacement_parses(&fixture("go"));
}

/// The fixture's `pricing.go`, reformatted: indentation and blank lines change.
fn relaid(indent: &str, blank: &str) -> String {
    std::fs::read_to_string(fixture("go").join("pricing.go"))
        .unwrap()
        .lines()
        .map(|line| {
            let depth = line.len() - line.trim_start_matches('\t').len();
            format!("{}{}", indent.repeat(depth), line.trim_start())
        })
        .collect::<Vec<_>>()
        .join(&format!("\n{blank}"))
}

fn site_hashes(root: &Path) -> Vec<(String, Vec<String>)> {
    let symbols: Symbols = produce("go", root);
    let sites: MutationSites = produce("go", root);
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
        indent in prop::sample::select(vec!["\t", "    ", "  ", "\t\t"]),
        blank in prop::sample::select(vec!["", "\n"]),
        prefix in prop::sample::select(vec!["", "// moved\n\n", "func earlier() {}\n\n"]),
        path in prop::sample::select(vec!["pricing.go", "money/pricing.go", "internal/p.go"]),
    ) {
        let original = tree(&[(
            "pricing.go",
            &std::fs::read_to_string(fixture("go").join("pricing.go")).unwrap(),
        )]);
        let laid_out = relaid(indent, blank);
        let (package, rest) = laid_out.split_once('\n').unwrap();
        let moved = tree(&[(path, &format!("{package}\n{prefix}{rest}\n"))]);
        let mut expected = site_hashes(original.path());
        let mut actual = site_hashes(moved.path());
        actual.retain(|(name, _)| name != "earlier");
        expected.sort();
        actual.sort();
        prop_assert_eq!(actual, expected);
        every_replacement_parses(moved.path());
    }
}
