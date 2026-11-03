// ABOUTME: The bound on the socket paths tests make Cupel create (ADR-159): macOS limits a Unix socket's path to 103
// ABOUTME: characters, and a test fails at more than 100, so wherever Cupel's own runs place a test's repository it fits.
use std::path::Path;

/// The longest socket path a test may make Cupel create: three under macOS's 103-character limit, so a path that
/// only just fits fails here first.
pub const SOCKET_PATH_MAX: usize = 100;

/// Fails when `socket`, as the daemon binds it, is longer than [`SOCKET_PATH_MAX`]. Every test that makes Cupel
/// create a socket calls it first.
///
/// # Panics
///
/// When `socket` is longer than [`SOCKET_PATH_MAX`].
pub fn assert_socket_fits(socket: &Path) {
    let length = socket.as_os_str().len();
    assert!(
        length <= SOCKET_PATH_MAX,
        "{} is {length} characters, over the {SOCKET_PATH_MAX} a socket path may take (macOS allows 103)",
        socket.display()
    );
}
