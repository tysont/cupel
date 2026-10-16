// ABOUTME: Sites become mutants (ADR-080 §1): one per site and replacement, its source text replaced at the site's
// ABOUTME: span, for every language's real `MutationSites`; a span whose text is not the site's makes none.
use cupel_core::syntax::{MutationOperator, OperatorSet, Site, Span};
use cupel_mutate::mutant::{self, apply, replacements};

fn site(from: &str, to: &[&str], span: Span) -> Site {
    Site {
        operator: MutationOperator::Relational,
        set: OperatorSet::Default,
        from: from.to_owned(),
        to: to.iter().map(|to| (*to).to_owned()).collect(),
        site_hash: "00000000000000aa".parse().unwrap(),
        span,
    }
}

fn span(line: u32, col: u32, end_line: u32, end_col: u32) -> Span {
    Span {
        line,
        col,
        end_line,
        end_col,
    }
}

#[test]
fn a_replacement_is_applied_at_the_sites_byte_span() {
    let source = "def f(x):\n    return x < 10\n";
    assert_eq!(
        apply(source, &span(2, 14, 2, 15), "<", "<="),
        Some("def f(x):\n    return x <= 10\n".to_owned())
    );
}

#[test]
fn columns_count_bytes_not_characters() {
    let source = "let s = \"é\"; s.len() > 0\n";
    let at = source.find('>').unwrap();
    let col = u32::try_from(at + 1).unwrap();
    assert_eq!(
        apply(source, &span(1, col, 1, col + 1), ">", ">="),
        Some("let s = \"é\"; s.len() >= 0\n".to_owned())
    );
}

#[test]
fn a_span_that_does_not_hold_the_sites_text_makes_no_mutant() {
    let source = "x = a + b\n";
    assert_eq!(apply(source, &span(1, 7, 1, 8), "-", "+"), None);
    assert_eq!(apply(source, &span(9, 1, 9, 2), "+", "-"), None);
}

#[test]
fn every_replacement_of_a_site_is_its_own_mutant() {
    let site = site("<", &["<=", ">="], span(1, 3, 1, 4));
    let made: Vec<String> = replacements("a < b\n", &site)
        .into_iter()
        .map(|(to, source)| format!("{to}: {}", source.trim_end()))
        .collect();
    assert_eq!(made, ["<=: a <= b", ">=: a >= b"]);
}

#[test]
fn a_byte_range_becomes_the_span_that_holds_it() {
    let source = "def f(x):\n    return x < 10\n";
    let at = source.find('<').unwrap();
    assert_eq!(mutant::span(source, at, at + 1), span(2, 14, 2, 15));
    assert_eq!(mutant::span(source, 0, source.len()), span(1, 1, 3, 1));
}
