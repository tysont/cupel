// ABOUTME: A test file of the same directory with its own `tool`, a helper that asserts only that the program ran,
// ABOUTME: which a call of `support::tool` in `cli.rs` is not.
use std::process::Command;

fn tool(args: &[&str]) -> bool {
    let status = Command::new("true").args(args).status().unwrap();
    assert!(status.success());
    status.success()
}

#[test]
fn pages_render() {
    assert_eq!(tool(&["pages"]), true);
}
