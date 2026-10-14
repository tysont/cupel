// ABOUTME: Suppressions: the `.cupel/suppressions.toml` format, `--until` dates and refs, and how an
// ABOUTME: active or expired suppression changes an envelope's findings and summary (ADR-047).
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::envelope::{Envelope, SeverityCounts};
use crate::error::{Error, Result};
use crate::finding::{Action, ActionKind, Evidence, Finding, Severity};
use crate::grade::Tier;
use crate::hash::FindingId;
use crate::id::CheckId;

/// When a suppression stops applying.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Until {
    /// The last day it applies, `YYYY-MM-DD` in UTC.
    Date(String),
    /// A git ref; the suppression expires once the ref's commit is an ancestor of `HEAD`.
    Ref(String),
}

impl Until {
    /// Whether this is a date earlier than `today` (`YYYY-MM-DD`). Refs are never past by date.
    pub fn date_passed(&self, today: &str) -> bool {
        match self {
            Self::Date(date) => today > date.as_str(),
            Self::Ref(_) => false,
        }
    }
}

impl fmt::Display for Until {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Date(text) | Self::Ref(text) => f.write_str(text),
        }
    }
}

impl FromStr for Until {
    type Err = Error;

    /// A value shaped like `NNNN-NN-NN` must be a valid date; anything else without whitespace
    /// is a ref.
    fn from_str(value: &str) -> Result<Self> {
        let invalid = |expected| Error::InvalidValue {
            what: "--until",
            value: value.to_owned(),
            expected,
        };
        if is_date_shaped(value) {
            return if is_valid_date(value) {
                Ok(Self::Date(value.to_owned()))
            } else {
                Err(invalid("a real date as YYYY-MM-DD"))
            };
        }
        if value.is_empty() || value.chars().any(char::is_whitespace) {
            return Err(invalid("a date as YYYY-MM-DD or a git ref"));
        }
        Ok(Self::Ref(value.to_owned()))
    }
}

impl TryFrom<String> for Until {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        value.parse()
    }
}

impl From<Until> for String {
    fn from(value: Until) -> Self {
        value.to_string()
    }
}

fn is_date_shaped(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(index, byte)| {
            if index == 4 || index == 7 {
                *byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
}

/// A date-shaped value whose month is 1–12 and whose day exists in that month.
fn is_valid_date(value: &str) -> bool {
    let number = |range: std::ops::Range<usize>| value[range].parse::<u32>().unwrap_or(0);
    let (year, month, day) = (number(0..4), number(5..7), number(8..10));
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

/// One suppressed finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suppression {
    pub finding: FindingId,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<Until>,
    /// The day it was written, `YYYY-MM-DD`.
    pub created: String,
}

/// The contents of `.cupel/suppressions.toml`, sorted by finding ID.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Suppressions {
    entries: Vec<Suppression>,
}

/// The file's serialized shape: an array of `[[suppression]]` tables.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SuppressionFile {
    #[serde(default)]
    suppression: Vec<Suppression>,
}

impl Suppressions {
    /// The file's path relative to `.cupel/`.
    pub const FILE: &'static str = "suppressions.toml";
    const HEADER: &'static str = "# Cupel suppressions: written by `cupel suppress <finding-id> --reason TEXT [--until DATE|REF]`.\n";

    /// Parses the file's text.
    pub fn parse(text: &str) -> Result<Self> {
        let file: SuppressionFile = toml::from_str(text).map_err(|error| Error::InvalidValue {
            what: "suppressions file",
            value: error.message().to_owned(),
            expected: "`[[suppression]]` tables with finding, reason, created, and optional until",
        })?;
        let mut suppressions = Self::default();
        for entry in file.suppression {
            suppressions.upsert(entry);
        }
        Ok(suppressions)
    }

    /// The file's text: a header comment, then one `[[suppression]]` table per entry.
    ///
    /// # Panics
    ///
    /// Never: every field is a string, which TOML always represents.
    pub fn to_toml(&self) -> String {
        let file = SuppressionFile {
            suppression: self.entries.clone(),
        };
        let body = toml::to_string(&file).expect("suppressions always serialize");
        format!("{}{body}", Self::HEADER)
    }

    /// Adds `entry`, replacing any entry for the same finding.
    pub fn upsert(&mut self, entry: Suppression) {
        match self
            .entries
            .binary_search_by(|existing| existing.finding.cmp(&entry.finding))
        {
            Ok(index) => self.entries[index] = entry,
            Err(index) => self.entries.insert(index, entry),
        }
    }

    pub fn entries(&self) -> &[Suppression] {
        &self.entries
    }
}

/// Applies `suppressions` to `envelope` (ADR-047 §4): each finding with an active suppression
/// becomes `suppressed` with severity `info`; each finding whose suppression `expired` is kept
/// as it is and reported by an added `suppression.expired` finding. Severity counts,
/// `summary.suppressed`, and `total_findings` are recounted. Findings already suppressed are
/// left alone, so applying twice changes nothing more.
pub fn apply_suppressions(
    envelope: &mut Envelope,
    suppressions: &[Suppression],
    expired: impl Fn(&Until) -> bool,
) {
    let mut reports = Vec::new();
    for finding in &mut envelope.findings {
        if finding.suppressed {
            continue;
        }
        let Some(suppression) = suppressions.iter().find(|s| s.finding == finding.id) else {
            continue;
        };
        match &suppression.until {
            Some(until) if expired(until) => reports.push(expiry(suppression, until, finding)),
            _ => {
                finding.suppressed = true;
                finding.severity = Severity::Info;
            }
        }
    }
    let known: Vec<FindingId> = envelope.findings.iter().map(|f| f.id.clone()).collect();
    envelope.findings.extend(
        reports
            .into_iter()
            .filter(|report| !known.contains(&report.id)),
    );
    let mut counts = SeverityCounts::default();
    for finding in &envelope.findings {
        match finding.severity {
            Severity::Fail => counts.fail += 1,
            Severity::Warn => counts.warn += 1,
            Severity::Info => counts.info += 1,
        }
    }
    envelope.summary.findings = counts;
    envelope.summary.suppressed = envelope.findings.iter().filter(|f| f.suppressed).count() as u64;
    envelope.total_findings = envelope.findings.len() as u64;
}

/// The `suppression.expired` finding for `suppression`, which `until` ended, of `finding`.
fn expiry(suppression: &Suppression, until: &Until, finding: &Finding) -> Finding {
    let check = CheckId::from("suppress");
    let kind = "suppression.expired";
    let subject = suppression.finding.to_string();
    let id = FindingId::for_subject(&check, kind, &subject);
    let evidence = json!({
        "finding": subject,
        "reason": suppression.reason,
        "until": until.to_string(),
    });
    Finding {
        suppress: format!("cupel suppress {id} --reason '…'"),
        id,
        check,
        kind: kind.to_owned(),
        severity: Severity::Warn,
        confidence: 1.0,
        tier: Tier::try_from(0).expect("tier 0 is valid"),
        symbol: None,
        location: None,
        title: format!(
            "The suppression of {} expired ({until}): {}",
            suppression.finding, finding.title
        ),
        detail: Some(format!("It was suppressed because: {}", suppression.reason)),
        evidence: Evidence(match evidence {
            Value::Object(map) => map,
            _ => Map::new(),
        }),
        action: Action {
            kind: ActionKind::Review,
            priority: 1,
            summary: format!(
                "Fix {}, or suppress it again with a later --until.",
                suppression.finding
            ),
            hints: Map::new(),
        },
        repro: finding.repro.clone(),
        new_since_baseline: false,
        suppressed: false,
    }
}
