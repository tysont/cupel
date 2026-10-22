// ABOUTME: The name of the named pipe behind a daemon's endpoint on Windows (ADR-137), derived from the endpoint's
// ABOUTME: absolute path; a pure function, so it is built and tested on every platform.
use std::path::Path;

/// The named pipe a daemon whose endpoint file is `endpoint` listens on under Windows:
/// `\\.\pipe\cupel-` and the first 16 hex digits of BLAKE3 of the endpoint's absolute path.
pub fn pipe_name(endpoint: &Path) -> String {
    let absolute = std::path::absolute(endpoint).unwrap_or_else(|_| endpoint.to_path_buf());
    let digest = blake3::hash(absolute.to_string_lossy().as_bytes()).to_hex();
    format!(r"\\.\pipe\cupel-{}", &digest[..16])
}
