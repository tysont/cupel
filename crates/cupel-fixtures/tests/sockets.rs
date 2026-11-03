// ABOUTME: The socket-path bound every test that makes Cupel create a socket checks (ADR-159): at most 100 characters,
// ABOUTME: three under macOS's limit, so a socket wherever Cupel's own runs place a test's repository fits with room.
use std::path::PathBuf;

use cupel_fixtures::{SOCKET_PATH_MAX, assert_socket_fits};

/// A path of `length` characters ending in `serve.sock`.
fn socket(length: usize) -> PathBuf {
    let name = "/serve.sock";
    PathBuf::from(format!("/{}{name}", "d".repeat(length - name.len() - 1)))
}

#[test]
fn a_socket_path_of_the_bound_fits() {
    assert_eq!(socket(SOCKET_PATH_MAX).as_os_str().len(), 100);
    assert_socket_fits(&socket(SOCKET_PATH_MAX));
}

#[test]
#[should_panic(expected = "is 101 characters, over the 100 a socket path may take")]
fn a_socket_path_over_the_bound_fails() {
    assert_socket_fits(&socket(SOCKET_PATH_MAX + 1));
}
