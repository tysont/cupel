// ABOUTME: The daemon protocol (ADR-009): requests and replies as JSON objects tagged by `type`, one per
// ABOUTME: line, at most `MAX_MESSAGE` bytes; reading reports malformed, cut-short, and oversized lines.
use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// The protocol version every request carries.
pub const PROTOCOL: u32 = 1;

/// The longest message, newline included (ADR-009 §2).
pub const MAX_MESSAGE: usize = 64 * 1024 * 1024;

/// Which `cupel` sent or serves a request: its version and a fingerprint of its executable, so a
/// rebuilt binary of the same version is a different build (ADR-085 §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Build {
    pub version: String,
    /// The executable's size and modification time (`<bytes>-<nanoseconds>`); empty when it
    /// cannot be read.
    pub binary: String,
}

impl Build {
    /// `version` built as the executable at `executable`.
    pub fn of(version: &str, executable: &Path) -> Self {
        let binary = std::fs::metadata(executable)
            .ok()
            .and_then(|metadata| {
                let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
                Some(format!("{}-{}", metadata.len(), modified.as_nanos()))
            })
            .unwrap_or_default();
        Self {
            version: version.to_owned(),
            binary,
        }
    }

    /// `version` built as this process's executable.
    pub fn current(version: &str) -> Self {
        std::env::current_exe().map_or_else(
            |_| Self::of(version, Path::new("")),
            |executable| Self::of(version, &executable),
        )
    }
}

impl fmt::Display for Build {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cupel {} (binary {})", self.version, self.binary)
    }
}

/// One CLI invocation as its process saw it: what `cupel_cli::Environment` holds, as text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invocation {
    /// The arguments after the program name.
    pub args: Vec<String>,
    pub current_dir: Option<PathBuf>,
    pub stdout_is_terminal: bool,
    /// The `CUPEL_*` variables.
    pub variables: BTreeMap<String, String>,
    /// `NO_COLOR` is set to a non-empty value.
    pub no_color: bool,
}

/// A request: the protocol, the client's build, and what it asks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub protocol: u32,
    pub build: Build,
    #[serde(flatten)]
    pub body: Body,
}

impl Request {
    /// `body` from `build`, in this protocol version.
    pub fn new(build: Build, body: Body) -> Self {
        Self {
            protocol: PROTOCOL,
            build,
            body,
        }
    }
}

/// What a request asks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Body {
    /// Run `invocation`; `path` is the client's `PATH`.
    Run {
        path: Option<String>,
        invocation: Invocation,
    },
    /// Say what the daemon is.
    Status,
    /// Exit.
    Stop,
}

/// A reply line. Every exchange ends with one that is not [`Reply::Stderr`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Reply {
    /// Text the run wrote to standard error while running, in order.
    Stderr {
        text: String,
    },
    Done(Done),
    Status(Status),
    Stopping,
    /// The daemon will not run the request; `reason` is a sentence for a person.
    Refused {
        reason: String,
    },
    /// The request was malformed.
    Error {
        message: String,
    },
}

/// What a cold process would have written and exited with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Done {
    pub stdout: String,
    /// What the run had not already streamed as [`Reply::Stderr`].
    pub stderr: String,
    pub code: u8,
}

/// What a daemon is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub version: String,
    pub build: Build,
    pub pid: u32,
    pub root: PathBuf,
    pub socket: PathBuf,
    /// Whether the file watcher runs (`serve --watch`).
    pub watching: bool,
    /// Requests run so far.
    pub served: u64,
    /// Files the watcher reparsed so far.
    pub reparsed: u64,
}

/// Why a line did not read as a message.
#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    /// The line was longer than `limit` bytes.
    TooLong {
        limit: usize,
    },
    /// The input ended inside a line.
    Truncated,
    /// The line is not the JSON of a message.
    Malformed(String),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "the connection failed: {error}"),
            Self::TooLong { limit } => {
                write!(f, "the message is longer than the limit of {limit} bytes")
            }
            Self::Truncated => f.write_str("the connection ended inside a message"),
            Self::Malformed(error) => write!(f, "the message is not valid: {error}"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Writes `message` as one line and flushes it.
pub fn write_message(out: &mut impl Write, message: &impl Serialize) -> io::Result<()> {
    let mut line = serde_json::to_vec(message).map_err(io::Error::other)?;
    line.push(b'\n');
    out.write_all(&line)?;
    out.flush()
}

/// Reads the next message; `None` when the input ends before its first byte.
pub fn read_message<T: DeserializeOwned>(
    input: &mut impl BufRead,
) -> Result<Option<T>, FrameError> {
    read_message_within(input, MAX_MESSAGE)
}

/// [`read_message`] with a limit of `limit` bytes, newline included.
pub fn read_message_within<T: DeserializeOwned>(
    input: &mut impl BufRead,
    limit: usize,
) -> Result<Option<T>, FrameError> {
    let Some(line) = read_line_within(input, limit)? else {
        return Ok(None);
    };
    serde_json::from_slice(&line)
        .map(Some)
        .map_err(|error| FrameError::Malformed(error.to_string()))
}

/// Reads the next line, newline included, of at most `limit` bytes; `None` when the input ends
/// before its first byte.
pub(crate) fn read_line_within(
    input: &mut impl BufRead,
    limit: usize,
) -> Result<Option<Vec<u8>>, FrameError> {
    let mut line = Vec::new();
    let allowed = u64::try_from(limit).unwrap_or(u64::MAX);
    input
        .take(allowed)
        .read_until(b'\n', &mut line)
        .map_err(FrameError::Io)?;
    if line.is_empty() {
        return Ok(None);
    }
    if line.last() != Some(&b'\n') {
        return Err(if line.len() >= limit {
            FrameError::TooLong { limit }
        } else {
            FrameError::Truncated
        });
    }
    Ok(Some(line))
}
