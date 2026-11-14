// ABOUTME: Checks the greeting against text a `str!` macro names, in a `let` and in an assertion's argument,
// ABOUTME: as snapbox's `str!` is used; the macro is local, so the crate needs no dependency.
use greeting::greet;

macro_rules! str {
    [[$text:expr]] => {
        $text.trim()
    };
    ($text:expr) => {
        $text
    };
}

#[test]
fn greets_by_name() {
    let expected = str!("Hello, Ada!");
    assert_eq!(greet("Ada"), expected);
}

#[test]
fn greets_with_a_raw_string() {
    assert_eq!(
        greet("Grace"),
        str![[r#"
Hello, Grace!
"#]]
    );
}
