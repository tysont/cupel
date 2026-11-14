// ABOUTME: Tests that check the parser rejects input by unwrapping its error, and one that only unwraps
// ABOUTME: its values, which asserts nothing.
use digits::parse;

#[test]
fn a_sign_is_rejected() {
    parse("+").unwrap_err();
    parse("1").unwrap();
}

#[test]
fn two_digits_are_rejected() {
    parse("12").expect_err("one digit at a time");
}

#[test]
fn digits_parse() {
    parse("1").unwrap();
    parse("2").expect("a digit");
}
