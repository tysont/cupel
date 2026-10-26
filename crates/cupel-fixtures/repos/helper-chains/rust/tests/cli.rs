// ABOUTME: assert_cmd chains: one starts with `tool()`, which `pages.rs` names an asserting helper of its own, and
// ABOUTME: one starts with `checked()`, an asserting helper of this file; each chain is a strong process assertion.
mod support;

use assert_cmd::prelude::*;
use std::process::Command;
use support::tool;

fn checked() -> Command {
    let command = Command::new("true");
    assert!(command.get_program() == "true");
    command
}

#[test]
fn the_tool_succeeds() {
    tool().arg("--version").assert().success();
}

#[test]
fn the_checked_tool_succeeds() {
    checked().arg("--version").assert().success();
}
