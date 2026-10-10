// ABOUTME: A finding about a named subject rather than a symbol (ADR-016). CLI errors and `doctor`
// ABOUTME: problems share this shape: tier 0, confidence 1, no symbol or location, the run's repro.
use cupel_core::{Action, ActionKind, CheckId, Evidence, Finding, FindingId, Severity, Tier};
use serde_json::{Map, Value};

/// Everything that varies between subject findings.
pub struct Problem {
    /// The emitting component, such as `cli` or `doctor`.
    pub check: &'static str,
    pub kind: &'static str,
    /// The structural name the finding ID is derived from: a flag, key, or tool ID.
    pub subject: String,
    pub severity: Severity,
    pub title: String,
    pub detail: Option<String>,
    /// A JSON object; anything else becomes an empty object.
    pub evidence: Value,
    pub action: ActionKind,
    /// What to do, the action's summary.
    pub fix: String,
}

impl Problem {
    /// The finding; `repro` is the command line that produced it.
    pub fn finding(self, repro: &str) -> Finding {
        let check = CheckId::from(self.check);
        let id = FindingId::for_subject(&check, self.kind, &self.subject);
        Finding {
            suppress: format!("cupel suppress {id} --reason '…'"),
            id,
            check,
            kind: self.kind.to_owned(),
            severity: self.severity,
            confidence: 1.0,
            tier: Tier::try_from(0).expect("tier 0 is valid"),
            symbol: None,
            location: None,
            title: self.title,
            detail: self.detail,
            evidence: Evidence(match self.evidence {
                Value::Object(map) => map,
                _ => Map::new(),
            }),
            action: Action {
                kind: self.action,
                priority: 1,
                summary: self.fix,
                hints: Map::new(),
            },
            repro: repro.to_owned(),
            new_since_baseline: false,
            suppressed: false,
        }
    }
}
