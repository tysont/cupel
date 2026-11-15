// ABOUTME: Tests that assert only through a `debug_assert…` method, an `assert…` method, or a `debug_assert!`
// ABOUTME: macro, beside one that asserts nothing (ADR-224).
use commands::Command;

#[test]
fn the_command_is_configured() {
    Command::new("serve").debug_assert();
}

#[test]
fn the_command_keeps_its_name() {
    Command::new("serve").assert_named("serve");
}

#[test]
fn the_name_is_not_empty() {
    debug_assert!(!Command::new("serve").name().is_empty());
}

#[test]
fn the_command_builds() {
    Command::new("serve");
}
