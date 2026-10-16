// ABOUTME: The client side of the daemon protocol: one request per connection, `stderr` lines handed over as
// ABOUTME: they arrive, and every way of not getting an answer told apart (no daemon, no answer, refused).
use std::fmt;
use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::path::Path;

use crate::protocol::{FrameError, Reply, Request, read_message, write_message};

/// Why a request got no final reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    /// There is no socket file: no daemon was started.
    Absent,
    /// The socket file exists but nothing answered on it: a daemon that is gone left it behind.
    Unanswered(String),
    /// The connection ended before the final reply: the daemon stopped or failed mid-request.
    Interrupted(String),
    /// The daemon declined to run the request (ADR-085 §4–5).
    Refused(String),
    /// A message on either side was not valid protocol.
    Protocol(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent => f.write_str("no daemon is running"),
            Self::Unanswered(why) => write!(f, "the daemon did not answer: {why}"),
            Self::Interrupted(why) => write!(f, "the daemon stopped answering: {why}"),
            Self::Refused(reason) => f.write_str(reason),
            Self::Protocol(why) => write!(f, "the daemon and this client disagree: {why}"),
        }
    }
}

impl std::error::Error for ClientError {}

/// Sends `request` to the daemon at `socket` and returns its final reply, handing each
/// [`Reply::Stderr`] text to `on_stderr` as it arrives. The connection stays open until the
/// final reply, since the daemon cancels a run whose client hangs up (ADR-009 §3).
pub fn request(
    socket: &Path,
    request: &Request,
    on_stderr: &mut dyn FnMut(&str),
) -> Result<Reply, ClientError> {
    if !socket.exists() {
        return Err(ClientError::Absent);
    }
    let unanswered = |error: &dyn fmt::Display| ClientError::Unanswered(error.to_string());
    let stream = UnixStream::connect(socket).map_err(|error| unanswered(&error))?;
    write_message(&mut &stream, request).map_err(|error| unanswered(&error))?;
    let mut replies = BufReader::new(&stream);
    let interrupted = |error: &dyn fmt::Display| ClientError::Interrupted(error.to_string());
    loop {
        match read_message::<Reply>(&mut replies) {
            Ok(None) => {
                return Err(interrupted(&"the connection ended before the final reply"));
            }
            Err(error @ (FrameError::Io(_) | FrameError::Truncated)) => {
                return Err(interrupted(&error));
            }
            Err(error) => return Err(ClientError::Protocol(error.to_string())),
            Ok(Some(Reply::Stderr { text })) => on_stderr(&text),
            Ok(Some(Reply::Refused { reason })) => return Err(ClientError::Refused(reason)),
            Ok(Some(Reply::Error { message })) => return Err(ClientError::Protocol(message)),
            Ok(Some(reply)) => return Ok(reply),
        }
    }
}
