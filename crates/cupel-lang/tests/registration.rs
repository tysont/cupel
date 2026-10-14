// ABOUTME: How generic producers join a language (ADR-074): each fact module submits its own, so
// ABOUTME: `generic.rs` attaches whatever was submitted and names no producer itself.

/// `generic.rs` as written, so the test sees exactly what a new generic producer would edit.
/// Comments may name producers; code may not.
const GENERIC: &str = include_str!("../src/generic.rs");

#[test]
fn generic_attaches_submissions_and_registers_no_producer_itself() {
    let named: Vec<&str> = GENERIC
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains("::extract") || line.contains(".register::<"))
        .collect();
    assert_eq!(
        named,
        Vec::<&str>::new(),
        "a generic producer submits itself from its fact module"
    );
}
