// ABOUTME: The `--events` destination: standard error or a file opened through the `Writer`, each receiving one
// ABOUTME: JSON line per event (design §7.4, ADR-020 §4). Without `--events`, the invocation's own stream, if any.
use std::io::Write;
use std::path::Path;

use cupel_core::{Error, Event, EventSink, JsonlSink, WriteError, Writer};

use crate::error::{CupelError, Usage};
use crate::session::Stream;

/// The value of `--events` that streams to standard error.
const STDERR: &str = "stderr";

/// Where this invocation's events go.
pub struct Events {
    sink: Option<JsonlSink<Box<dyn Write>>>,
}

impl Events {
    /// Opens `--events`: `stderr` (the invocation's standard error stream), or a path resolved
    /// against the root and checked by `writer`; without it, `otherwise` (a tool call's progress
    /// stream under `serve --mcp`) or nowhere.
    pub fn open(
        destination: Option<&str>,
        writer: &Writer,
        (stderr, otherwise): (&Stream, Option<&Stream>),
    ) -> Result<Self, CupelError> {
        let out: Box<dyn Write> = match (destination, otherwise) {
            (None, None) => return Ok(Self { sink: None }),
            (None, Some(stream)) => Box::new(stream.clone()),
            (Some(STDERR), _) => Box::new(stderr.clone()),
            (Some(path), _) => Box::new(
                writer
                    .stream(Path::new(path))
                    .map_err(|error| refused(path, &error))?,
            ),
        };
        Ok(Self {
            sink: Some(JsonlSink::new(out)),
        })
    }

    /// Why the stream stopped early, if it did.
    pub fn error(&self) -> Option<&str> {
        self.sink.as_ref().and_then(JsonlSink::error)
    }
}

impl EventSink for Events {
    fn emit(&mut self, event: Event) {
        if let Some(sink) = &mut self.sink {
            sink.emit(event);
        }
    }
}

/// The usage error for an events file that may not or cannot be written.
fn refused(path: &str, error: &Error) -> CupelError {
    let fix = match error {
        Error::Write(WriteError::Outside { .. }) => {
            "Pass --write to permit writing the events file there, put it under .cupel/, or use \
             --events stderr."
        }
        _ => "Choose a writable path for --events, or use --events stderr.",
    };
    Usage {
        subject: "--events".to_owned(),
        title: error.to_string(),
        fix: fix.to_owned(),
        detail: None,
        argument: Some("--events".to_owned()),
        value: Some(path.to_owned()),
        valid: Vec::new(),
    }
    .into()
}
