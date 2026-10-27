// ABOUTME: The command every test of `cli.rs` starts from, shared as `mod support`; it asserts nothing.
// ABOUTME: Its name is the name of an asserting helper of `pages.rs`.
use std::process::Command;

pub fn tool() -> Command {
    Command::new("true")
}
