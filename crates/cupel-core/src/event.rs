// ABOUTME: The `--events` stream: one JSON object per line, tagged by `ev` (design §7.4).
// ABOUTME: Also defines `EventSink`, the destination checks and producers report through, and `JsonlSink`.
use std::io::Write;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::finding::Finding;
use crate::id::{CheckId, RunId};
use crate::rank::Measurement;
use crate::result::{CheckStatus, SkipReason};
use crate::status::{ExitCode, Status};

/// A progress event. Findings stream as they are discovered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "ev")]
pub enum Event {
    #[serde(rename = "plan")]
    Plan {
        run: RunId,
        checks: u32,
        est_ms: u64,
    },
    #[serde(rename = "check.start")]
    CheckStart { id: CheckId, targets: u64 },
    #[serde(rename = "finding")]
    Finding(Box<Finding>),
    #[serde(rename = "progress")]
    Progress {
        id: CheckId,
        done: u64,
        total: u64,
        eta_ms: Option<u64>,
    },
    #[serde(rename = "check.end")]
    CheckEnd {
        id: CheckId,
        status: CheckStatus,
        duration_ms: u64,
    },
    #[serde(rename = "skip")]
    Skip { id: CheckId, reason: SkipReason },
    #[serde(rename = "run.end")]
    RunEnd { status: Status, exit_code: ExitCode },
}

/// Where checks and producers send events. The core serializes them to `--events` and
/// accumulates the envelope.
pub trait EventSink {
    fn emit(&mut self, event: Event);

    /// Reports one per-target value for `survey`'s rankings (ADR-062). It is not an event and
    /// never reaches `--events`; sinks other than the scheduler's ignore it.
    fn measure(&mut self, _measurement: Measurement) {}
}

/// Collects events in order.
impl EventSink for Vec<Event> {
    fn emit(&mut self, event: Event) {
        self.push(event);
    }
}

/// Writes each event as one compact JSON line and flushes it, so a reader sees it at once. The
/// first write error stops the stream; [`error`](Self::error) reports it.
pub struct JsonlSink<W: Write> {
    out: W,
    error: Option<String>,
}

impl<W: Write> JsonlSink<W> {
    pub fn new(out: W) -> Self {
        Self { out, error: None }
    }

    /// Why the stream stopped, if it did.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn write_line(&mut self, event: &Event) -> std::io::Result<()> {
        let mut line = serde_json::to_vec(event).map_err(std::io::Error::other)?;
        line.push(b'\n');
        self.out.write_all(&line)?;
        self.out.flush()
    }
}

impl<W: Write> EventSink for JsonlSink<W> {
    fn emit(&mut self, event: Event) {
        if self.error.is_some() {
            return;
        }
        if let Err(error) = self.write_line(&event) {
            self.error = Some(error.to_string());
        }
    }
}
