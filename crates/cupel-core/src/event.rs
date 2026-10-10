// ABOUTME: The `--events` stream: one JSON object per line, tagged by `ev` (design §7.4).
// ABOUTME: Also defines `EventSink`, the destination checks and producers report through.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::envelope::{CheckStatus, ExitCode, SkipReason, Status};
use crate::finding::Finding;
use crate::id::{CheckId, RunId};

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
}

/// Collects events in order.
impl EventSink for Vec<Event> {
    fn emit(&mut self, event: Event) {
        self.push(event);
    }
}
