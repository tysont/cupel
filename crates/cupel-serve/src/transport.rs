// ABOUTME: The daemon's transport (ADR-137): a listener and a connection at an endpoint path, over a Unix socket
// ABOUTME: on Unix and over a named pipe behind an endpoint file on Windows, chosen at compile time.
//! Everything the server and client do with a connection goes through the listener and
//! [`Connection`], so the daemon names neither a socket nor a pipe. An endpoint is a path: on
//! Unix the socket file, on Windows a file naming the pipe ([`pipe_name`]). Either way a client
//! looks for the file, and the daemon ends when its file is removed or replaced (ADR-085 §9).
use std::path::Path;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::Connection;
#[cfg(unix)]
pub(crate) use unix::Listener;
#[cfg(windows)]
pub use windows::Connection;
#[cfg(windows)]
pub(crate) use windows::Listener;

/// The named pipe a daemon whose endpoint file is `endpoint` listens on under Windows:
/// `\\.\pipe\cupel-` and the first 16 hex digits of BLAKE3 of the endpoint's absolute path.
pub fn pipe_name(endpoint: &Path) -> String {
    let absolute = std::path::absolute(endpoint).unwrap_or_else(|_| endpoint.to_path_buf());
    let digest = blake3::hash(absolute.to_string_lossy().as_bytes()).to_hex();
    format!(r"\\.\pipe\cupel-{}", &digest[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pipe_is_named_for_the_endpoint_s_absolute_path() {
        let name = pipe_name(Path::new("/repo/.cupel/serve.sock"));
        let digits = name
            .strip_prefix(r"\\.\pipe\cupel-")
            .unwrap_or_else(|| panic!("not a Cupel pipe: {name:?}"));
        assert_eq!(digits.len(), 16, "{name}");
        assert!(digits.bytes().all(|b| b.is_ascii_hexdigit()), "{name}");
        assert_eq!(name, pipe_name(Path::new("/repo/.cupel/serve.sock")));
        assert_ne!(name, pipe_name(Path::new("/other/.cupel/serve.sock")));
    }

    #[test]
    fn a_relative_endpoint_names_the_pipe_of_its_absolute_path() {
        let relative = Path::new(".cupel/serve.sock");
        let absolute = std::env::current_dir().unwrap().join(relative);
        assert_eq!(pipe_name(relative), pipe_name(&absolute));
    }
}
