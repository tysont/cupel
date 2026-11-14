// ABOUTME: Helpers the test modules share: one checks a parse against the value expected, the other
// ABOUTME: only builds the inputs a test parses.
use digits::parse;

pub(crate) fn check_parse<T: Into<String>>(text: T, expected: u8) {
    let text = text.into();
    assert!(!text.is_empty());
    assert_eq!(parse(&text), Ok(expected));
}

pub(crate) fn inputs() -> Vec<&'static str> {
    vec!["0", "7", "9"]
}
