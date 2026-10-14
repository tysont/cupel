// ABOUTME: The plan a `--dry-run` returns (design §6.8): what would run, be skipped, be installed,
// ABOUTME: and be written, with an estimate. Carried in the envelope's nullable `plan` (ADR-020).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use similar::TextDiff;

use crate::envelope::Skipped;
use crate::id::{CheckId, ToolId};

/// Everything a run would do, without doing it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunPlan {
    pub would_run: Vec<PlannedCheck>,
    pub would_skip: Vec<PlannedSkip>,
    pub would_install: Vec<PlannedInstall>,
    pub would_write: Vec<PlannedWrite>,
    /// The sum of the planned checks' estimates.
    pub est_total_ms: u64,
}

/// A check the run would execute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlannedCheck {
    pub id: CheckId,
    pub targets: u64,
    pub est_ms: u64,
    /// The facts it needs and where they would come from, such as `coverage(cached)`.
    pub needs: Vec<String>,
    /// What the estimate rests on, such as how many targets are cached.
    pub note: Option<String>,
}

/// A check the run would skip, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlannedSkip {
    pub id: CheckId,
    #[serde(flatten)]
    pub skipped: Skipped,
}

/// A tool the run would install (`--install`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlannedInstall {
    pub tool: ToolId,
    /// The command that would install it, when one is known.
    pub command: Option<String>,
}

/// A file the run would write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlannedWrite {
    /// Relative to the repository root; absolute when outside it.
    pub path: String,
    /// A unified diff from the current content; null when the content is only known once the
    /// run happens, such as a run record.
    pub diff: Option<String>,
}

impl PlannedWrite {
    /// A write to `path` that changes the text named `label` from `before` (`None` when it does
    /// not exist yet) to `after`, as a unified diff with three lines of context and headers
    /// `a/<label>` (or `/dev/null`) and `b/<label>` (ADR-020 §6). `label` is the file itself for
    /// ordinary writes, or a part of it, such as a store row, for writes that are not text.
    pub fn change(path: &str, label: &str, before: Option<&str>, after: &str) -> Self {
        let old_header = before.map_or_else(|| "/dev/null".to_owned(), |_| format!("a/{label}"));
        let diff = TextDiff::from_lines(before.unwrap_or_default(), after)
            .unified_diff()
            .context_radius(3)
            .header(&old_header, &format!("b/{label}"))
            .to_string();
        Self {
            path: path.to_owned(),
            diff: Some(diff),
        }
    }
}
