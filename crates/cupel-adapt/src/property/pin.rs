// ABOUTME: Reads the case count a property test sets itself (ADR-183) from its source: fast-check's `numRuns`,
// ABOUTME: Hypothesis's `max_examples`, proptest's `with_cases` or `cases`, quickcheck's `tests`; the count when literal.
use cupel_core::Pinned;
use cupel_core::syntax::Span;

/// fast-check: `numRuns: 120` in the case, as `fc.assert`'s parameters write it.
pub fn fast_check(text: &str, span: &Span) -> Option<Pinned> {
    setting(&case_text(text, span), "numRuns", ':')
}

/// Hypothesis: `max_examples=500` in the case or a decorator above it (`@settings(…)`).
pub fn hypothesis(text: &str, span: &Span) -> Option<Pinned> {
    setting(&case_text(text, span), "max_examples", '=')
}

/// proptest: a configuration that sets `cases`, as `ProptestConfig::with_cases(10)` or
/// `cases: 10`, in the case's own `#[proptest(…)]` attribute or the `#![proptest_config(…)]` of
/// the `proptest!` block it is in.
pub fn proptest(text: &str, span: &Span) -> Option<Pinned> {
    configured(&case_text(text, span)).or_else(|| configured(enclosing_block(text, span)?))
}

/// quickcheck: `QuickCheck::new().tests(500)` in the case.
pub fn quickcheck(text: &str, span: &Span) -> Option<Pinned> {
    let case = case_text(text, span);
    if !case.contains("QuickCheck") {
        return None;
    }
    call(&case, ".tests")
}

/// The integer types a Rust literal may name as its suffix.
const SUFFIXES: [&str; 12] = [
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
];

/// The byte offset where line `line` (1-based) of `text` starts.
fn line_start(text: &str, line: u32) -> usize {
    text.split_inclusive('\n')
        .take(line.saturating_sub(1) as usize)
        .map(str::len)
        .sum()
}

/// The case's lines, with the lines of the decorators or attributes directly above it: each
/// line up to a blank one or one that ends a statement or opens a block.
fn case_text(text: &str, span: &Span) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let last = (span.end_line as usize).min(lines.len());
    let mut first = (span.line as usize).saturating_sub(1).min(last);
    while first > 0 && continues(lines[first - 1]) {
        first -= 1;
    }
    lines[first..last].join("\n")
}

/// Whether `line`, above a case, belongs to the case: not blank, and not the end of a statement
/// or the opening of a block.
fn continues(line: &str) -> bool {
    let line = line.trim();
    !line.is_empty() && !line.ends_with(['}', ';', ':', '{'])
}

/// The text from the `proptest!` block the case is in to the case; `None` when it is in none.
fn enclosing_block<'t>(text: &'t str, span: &Span) -> Option<&'t str> {
    let case = line_start(text, span.line).min(text.len());
    let at = text[..case].rfind("proptest!")?;
    let header = &text[at..case];
    let open = header.find('{')?;
    let depth = header[open..].chars().fold(0_i32, |depth, c| match c {
        '{' => depth + 1,
        '}' => depth - 1,
        _ => depth,
    });
    (depth > 0).then_some(header)
}

/// The cases a proptest configuration in `text` sets: in the parentheses of its first
/// `proptest_config(…)` or `#[proptest(…)]` that sets any.
fn configured(text: &str) -> Option<Pinned> {
    ["proptest_config(", "#[proptest("]
        .into_iter()
        .flat_map(|opening| {
            text.match_indices(opening)
                .map(move |(at, _)| at + opening.len())
        })
        .find_map(|inside| {
            let config = parenthesized(&text[inside..]);
            call(config, "with_cases")
                .or_else(|| setting(config, "cases", ':'))
                .or_else(|| setting(config, "cases", '='))
        })
}

/// The start of `text` up to the parenthesis that closes one opened just before it.
fn parenthesized(text: &str) -> &str {
    let mut depth = 0_u32;
    for (at, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => return &text[..at],
            ')' => depth -= 1,
            _ => {}
        }
    }
    text
}

/// The count of the first call `name(…)` in `text`.
fn call(text: &str, name: &str) -> Option<Pinned> {
    let opening = format!("{name}(");
    let (at, _) = text.match_indices(&opening).next()?;
    Some(Pinned {
        cases: count(&text[at + opening.len()..]),
    })
}

/// The count of the first `key` in `text`, as a whole word followed by `separator` and its value,
/// or, with `:`, an object's shorthand field (`{ key }`), whose count is unknown.
fn setting(text: &str, key: &str, separator: char) -> Option<Pinned> {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(key).find_map(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + key.len()..].trim_start();
        if before.is_some_and(word) || after.starts_with(word) {
            None
        } else if let Some(value) = after.strip_prefix(separator) {
            Some(Pinned {
                cases: count(value),
            })
        } else {
            (separator == ':' && after.starts_with([',', '}'])).then_some(Pinned { cases: None })
        }
    })
}

/// The integer literal `value` starts with, up to the end of its argument or field; `None` when
/// it is not one (a name or an expression).
fn count(value: &str) -> Option<u32> {
    let end = value
        .find([',', ')', '}', ';', '\n'])
        .unwrap_or(value.len());
    let literal = value[..end].trim().replace('_', "");
    let digits = literal
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(literal.len());
    let (number, suffix) = literal.split_at(digits);
    if !suffix.is_empty() && !SUFFIXES.contains(&suffix) {
        return None;
    }
    number.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The span of the lines from the one starting with `first` to the one starting with `last`.
    fn span_of(text: &str, first: &str, last: &str) -> Span {
        let line = |start: &str| {
            let index = text
                .lines()
                .position(|line| line.trim_start().starts_with(start))
                .unwrap();
            u32::try_from(index).unwrap() + 1
        };
        Span {
            line: line(first),
            col: 1,
            end_line: line(last),
            end_col: 1,
        }
    }

    fn counted(cases: u32) -> Pinned {
        Pinned { cases: Some(cases) }
    }

    const UNCOUNTED: Option<Pinned> = Some(Pinned { cases: None });

    #[test]
    fn fast_check_pins_by_num_runs() {
        let read = |assert: &str| {
            let text = format!(
                "test(\"digits\", () => {{\n  fc.assert(\n    fc.property(fc.nat(), (n) => digits(n) > 0),\n    {assert}\n  );\n}});\n"
            );
            fast_check(&text, &span_of(&text, "test(", "});"))
        };
        assert_eq!(read("{ numRuns: 120 }"), Some(counted(120)));
        assert_eq!(
            read("{ seed: 4, numRuns: 1_000, verbose: true }"),
            Some(counted(1000))
        );
        assert_eq!(read("{ numRuns: RUNS }"), UNCOUNTED);
        assert_eq!(read("{ numRuns }"), UNCOUNTED);
        assert_eq!(read("{ numRuns: 2 * 60 }"), UNCOUNTED);
        assert_eq!(read("{ seed: 4 }"), None);
        assert_eq!(read(""), None);
    }

    #[test]
    fn a_count_outside_the_case_pins_nothing() {
        let text = "test(\"a\", () => {\n  fc.assert(p, { numRuns: 5 });\n});\n\ntest(\"b\", () => {\n  fc.assert(p);\n});\n";
        assert_eq!(
            fast_check(text, &span_of(text, "test(\"b\"", "fc.assert(p);")),
            None
        );
    }

    #[test]
    fn hypothesis_pins_by_max_examples_in_its_settings() {
        let read = |settings: &str| {
            let text = format!(
                "import x\n\n{settings}\n@given(st.integers())\ndef test_digits(n):\n    assert digits(n) > 0\n"
            );
            hypothesis(&text, &span_of(&text, "def test_digits", "assert"))
        };
        assert_eq!(read("@settings(max_examples=500)"), Some(counted(500)));
        assert_eq!(
            read("@settings(\n    deadline=None,\n    max_examples = 50,\n)"),
            Some(counted(50))
        );
        assert_eq!(read("@settings(max_examples=EXAMPLES)"), UNCOUNTED);
        assert_eq!(read("@settings(deadline=None)"), None);
        assert_eq!(read("# no settings"), None);
    }

    #[test]
    fn proptest_pins_by_its_blocks_configuration_or_its_attribute() {
        let block = |config: &str| {
            format!(
                "proptest! {{\n    {config}\n    #[test]\n    fn digits(n in 0u32..10) {{\n        prop_assert!(n < 10);\n    }}\n}}\n"
            )
        };
        let read = |text: &str| proptest(text, &span_of(text, "fn digits", "prop_assert"));
        assert_eq!(
            read(&block(
                "#![proptest_config(ProptestConfig::with_cases(10))]"
            )),
            Some(counted(10))
        );
        assert_eq!(
            read(&block(
                "#![proptest_config(ProptestConfig {\n        cases: 64u32,\n        ..ProptestConfig::default()\n    })]"
            )),
            Some(counted(64))
        );
        assert_eq!(
            read(&block(
                "#![proptest_config(ProptestConfig::with_cases(CASES))]"
            )),
            UNCOUNTED
        );
        assert_eq!(
            read(&block(
                "#![proptest_config(ProptestConfig { max_shrink_iters: 5, ..ProptestConfig::default() })]"
            )),
            None
        );
        assert_eq!(read(&block("")), None);
        let attribute =
            "#[proptest(cases = 32)]\nfn digits(n: u32) {\n    prop_assert!(n < 10);\n}\n";
        assert_eq!(read(attribute), Some(counted(32)));
    }

    #[test]
    fn a_closed_proptest_block_does_not_configure_a_later_case() {
        let text = "proptest! {\n    #![proptest_config(ProptestConfig::with_cases(10))]\n    #[test]\n    fn other(n in 0u32..10) {\n        prop_assert!(n < 10);\n    }\n}\n\n#[proptest]\nfn digits(n: u32) {\n    prop_assert!(n < 10);\n}\n";
        let span = Span {
            line: 10,
            col: 1,
            end_line: 12,
            end_col: 2,
        };
        assert_eq!(proptest(text, &span), None);
        let first = proptest(text, &span_of(text, "fn other", "prop_assert"));
        assert_eq!(first, Some(counted(10)));
    }

    #[test]
    fn quickcheck_pins_by_its_tests() {
        let read = |call: &str| {
            let text = format!("#[test]\nfn digits() {{\n    {call}\n}}\n");
            quickcheck(&text, &span_of(&text, "fn digits", "}"))
        };
        assert_eq!(
            read("QuickCheck::new().tests(500).quickcheck(prop as fn(u32) -> bool);"),
            Some(counted(500))
        );
        assert_eq!(
            read("QuickCheck::new().tests(TESTS).quickcheck(prop as fn(u32) -> bool);"),
            UNCOUNTED
        );
        assert_eq!(read("quickcheck(prop as fn(u32) -> bool);"), None);
    }
}
