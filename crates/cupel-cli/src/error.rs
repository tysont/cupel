// ABOUTME: `CupelError`, the failure every verb can return, and its mapping to an exit code and a
// ABOUTME: `cli.*` finding: usage errors and unbuilt features exit 2, internal errors exit 6.
use clap::error::{ContextKind, ContextValue, ErrorKind};
use cupel_core::{
    Action, ActionKind, CheckId, Evidence, ExitCode, Finding, FindingId, Severity, Tier,
};
use serde_json::{Map, Value, json};

/// Why a verb did not produce a result. See ADR-017 for the findings each variant becomes.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CupelError {
    /// The command line or its inputs are wrong; the finding names the fix.
    #[error("{}", .0.title)]
    Usage(Box<Usage>),
    /// The command line asks for something a later plan step builds.
    #[error("{} is not implemented yet", .0.what)]
    NotImplemented(Pending),
    /// Cupel has a bug.
    #[error("internal error: {}", .0.message)]
    Internal(BugReport),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    /// The structural name the finding ID is derived from (ADR-016).
    pub subject: String,
    pub title: String,
    /// What to do instead.
    pub fix: String,
    pub detail: Option<String>,
    /// The argument at fault, such as `--format`.
    pub argument: Option<String>,
    /// The value supplied, when there was one.
    pub value: Option<String>,
    /// The values that would have been accepted, when they are enumerable.
    pub valid: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub subject: String,
    /// What was asked for, as the user would write it, such as `cupel check`.
    pub what: String,
    /// The implementation-plan step that builds it.
    pub step: String,
    pub fix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BugReport {
    pub message: String,
    /// Where in Cupel's source the failure happened.
    pub location: String,
    pub args: Vec<String>,
}

impl CupelError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage(_) | Self::NotImplemented(_) => ExitCode::Usage,
            Self::Internal(_) => ExitCode::Internal,
        }
    }

    /// A verb whose behavior arrives in `step`.
    pub fn pending_verb(words: &[String], step: &str) -> Self {
        Self::NotImplemented(Pending {
            subject: words.join(" "),
            what: format!("`cupel {}`", words.join(" ")),
            step: step.to_owned(),
            fix: "Use a verb that is implemented; `cupel describe` marks pending ones with \
                  pending_step."
                .to_owned(),
        })
    }

    /// An output format whose renderer arrives in `step`.
    pub fn pending_format(name: &str, step: &str) -> Self {
        Self::NotImplemented(Pending {
            subject: format!("--format {name}"),
            what: format!("`--format {name}`"),
            step: step.to_owned(),
            fix: "Use --format json or --format text.".to_owned(),
        })
    }

    /// A `describe` subject whose data arrives in `step`; `subject` names it, such as
    /// `describe matrix`.
    pub fn pending_description(subject: &str, step: &str) -> Self {
        Self::NotImplemented(Pending {
            subject: subject.to_owned(),
            what: format!("`cupel {subject}`"),
            step: step.to_owned(),
            fix: "Run `cupel describe --all` to see everything that can be described now."
                .to_owned(),
        })
    }

    /// The command line named no verb.
    pub fn missing_verb(verbs: Vec<String>) -> Self {
        Self::from(Usage {
            subject: "verb".to_owned(),
            title: "No verb given".to_owned(),
            fix: format!("Name a verb: {}.", verbs.join(", ")),
            detail: None,
            argument: None,
            value: None,
            valid: verbs,
        })
    }

    /// The finding that reports this error. `repro` is the command line that produced it.
    pub fn finding(&self, repro: &str) -> Finding {
        let (kind, subject, title, detail, evidence, action, fix) = match self {
            Self::Usage(usage) => (
                "cli.usage",
                usage.subject.as_str(),
                usage.title.clone(),
                usage.detail.clone(),
                json!({"argument": usage.argument, "value": usage.value, "valid": usage.valid}),
                ActionKind::Configure,
                usage.fix.clone(),
            ),
            Self::NotImplemented(pending) => (
                "cli.not-implemented",
                pending.subject.as_str(),
                self.to_string(),
                Some(format!("Implemented in plan step {}.", pending.step)),
                json!({"step": pending.step}),
                ActionKind::Review,
                pending.fix.clone(),
            ),
            Self::Internal(report) => (
                "cli.internal",
                report.location.as_str(),
                format!("Cupel failed internally: {}", report.message),
                Some(
                    "This is a bug in Cupel, not in the code under test. Please report it with \
                     the bug_report evidence."
                        .to_owned(),
                ),
                json!({"bug_report": {
                    "version": env!("CARGO_PKG_VERSION"),
                    "os": std::env::consts::OS,
                    "arch": std::env::consts::ARCH,
                    "message": report.message,
                    "location": report.location,
                    "args": report.args,
                    "report": concat!(env!("CARGO_PKG_REPOSITORY"), "/issues/new"),
                }}),
                ActionKind::Review,
                "Report this Cupel bug with the bug_report evidence attached.".to_owned(),
            ),
        };
        let check = CheckId::from("cli");
        let id = FindingId::for_subject(&check, kind, subject);
        Finding {
            suppress: format!("cupel suppress {id} --reason '…'"),
            id,
            check,
            kind: kind.to_owned(),
            severity: Severity::Fail,
            confidence: 1.0,
            tier: Tier::try_from(0).expect("tier 0 is valid"),
            symbol: None,
            location: None,
            title,
            detail,
            evidence: Evidence(object(evidence)),
            action: Action {
                kind: action,
                priority: 1,
                summary: fix,
                hints: Map::new(),
            },
            repro: repro.to_owned(),
            new_since_baseline: false,
            suppressed: false,
        }
    }
}

impl From<Usage> for CupelError {
    fn from(usage: Usage) -> Self {
        Self::Usage(Box::new(usage))
    }
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

impl Usage {
    /// Translates clap's parse error, naming the argument at fault and the fix. `verbs` lists
    /// the top-level verbs, offered when the verb itself is missing or unknown.
    pub fn from_clap(error: &clap::Error, verbs: &[String]) -> Self {
        let rendered = error.render().to_string();
        let title = rendered
            .lines()
            .next()
            .unwrap_or_default()
            .trim_start_matches("error: ")
            .to_owned();
        let argument = first(error, ContextKind::InvalidArg).map(|arg| argument_name(&arg));
        let value = first(error, ContextKind::InvalidValue);
        let mut valid = all(error, ContextKind::ValidValue);
        let fix = match error.kind() {
            ErrorKind::InvalidValue | ErrorKind::ValueValidation => {
                let arg = argument.clone().unwrap_or_default();
                if valid.is_empty() {
                    format!("Pass a valid value to {arg}; `cupel describe` gives its type.")
                } else {
                    format!("Pass one of: {} to {arg}.", valid.join(", "))
                }
            }
            ErrorKind::UnknownArgument => match first(error, ContextKind::SuggestedArg) {
                Some(suggestion) => format!("Did you mean {suggestion}?"),
                None => "Remove it; `cupel describe <verb>` lists the accepted flags.".to_owned(),
            },
            ErrorKind::InvalidSubcommand | ErrorKind::MissingSubcommand => {
                valid = all(error, ContextKind::ValidSubcommand);
                if valid.is_empty() {
                    valid = verbs.to_vec();
                }
                format!("Use one of: {}.", valid.join(", "))
            }
            ErrorKind::MissingRequiredArgument => {
                let missing = all(error, ContextKind::InvalidArg);
                format!("Pass {}.", missing.join(" and "))
            }
            ErrorKind::ArgumentConflict => {
                let prior = first(error, ContextKind::PriorArg)
                    .map(|arg| argument_name(&arg))
                    .unwrap_or_default();
                format!(
                    "Pass only one of {} and {prior}.",
                    argument.clone().unwrap_or_default()
                )
            }
            _ => "Run `cupel describe` for valid usage.".to_owned(),
        };
        let (subject, argument, value) = match error.kind() {
            ErrorKind::InvalidSubcommand => (
                "verb".to_owned(),
                None,
                first(error, ContextKind::InvalidSubcommand),
            ),
            ErrorKind::MissingSubcommand => {
                let parent =
                    first(error, ContextKind::InvalidSubcommand).filter(|parent| parent != "cupel");
                (parent.unwrap_or_else(|| "verb".to_owned()), None, None)
            }
            _ => (
                argument
                    .clone()
                    .unwrap_or_else(|| "command line".to_owned()),
                argument,
                value,
            ),
        };
        Self {
            subject,
            title,
            fix,
            detail: Some(rendered.trim_end().to_owned()),
            argument,
            value,
            valid,
        }
    }
}

/// `--format <FORMAT>` → `--format`; `<FINDING>` → `FINDING`; `[SUBJECT]...` → `SUBJECT`.
fn argument_name(rendered: &str) -> String {
    let head = rendered.split_whitespace().next().unwrap_or(rendered);
    head.trim_end_matches("...")
        .trim_matches(|c| matches!(c, '<' | '>' | '[' | ']'))
        .to_owned()
}

fn first(error: &clap::Error, kind: ContextKind) -> Option<String> {
    match error.get(kind)? {
        ContextValue::String(value) => Some(value.clone()),
        ContextValue::Strings(values) => values.first().cloned(),
        ContextValue::StyledStr(value) => Some(value.to_string()),
        _ => None,
    }
}

fn all(error: &clap::Error, kind: ContextKind) -> Vec<String> {
    match error.get(kind) {
        Some(ContextValue::Strings(values)) => values.clone(),
        Some(ContextValue::String(value)) => vec![value.clone()],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> BugReport {
        BugReport {
            message: "boom".to_owned(),
            location: "src/app.rs:1:1".to_owned(),
            args: vec!["describe".to_owned()],
        }
    }

    #[test]
    fn usage_and_pending_errors_exit_2_and_internal_errors_exit_6() {
        assert_eq!(
            CupelError::missing_verb(vec![]).exit_code(),
            ExitCode::Usage
        );
        assert_eq!(
            CupelError::pending_verb(&["check".to_owned()], "P2.1").exit_code(),
            ExitCode::Usage
        );
        assert_eq!(
            CupelError::Internal(report()).exit_code(),
            ExitCode::Internal
        );
    }

    #[test]
    fn findings_are_identified_by_check_kind_and_subject() {
        let finding = CupelError::pending_format("jsonl", "P0.5").finding("cupel x");
        assert_eq!(
            finding.id,
            FindingId::for_subject(
                &CheckId::from("cli"),
                "cli.not-implemented",
                "--format jsonl"
            )
        );
        assert_eq!(finding.repro, "cupel x");
        assert_eq!(finding.check.as_str(), "cli");
        assert_eq!(finding.action.kind, ActionKind::Review);
    }

    #[test]
    fn usage_findings_ask_to_configure_and_carry_argument_value_and_valid() {
        let error = CupelError::from(Usage {
            subject: "--format".to_owned(),
            title: "bad".to_owned(),
            fix: "fix it".to_owned(),
            detail: None,
            argument: Some("--format".to_owned()),
            value: Some("xml".to_owned()),
            valid: vec!["json".to_owned()],
        });
        let finding = error.finding("cupel");
        assert_eq!(finding.action.kind, ActionKind::Configure);
        assert_eq!(finding.action.summary, "fix it");
        assert_eq!(
            Value::Object(finding.evidence.0),
            json!({"argument": "--format", "value": "xml", "valid": ["json"]})
        );
    }

    #[test]
    fn argument_names_drop_value_placeholders() {
        assert_eq!(argument_name("--format <FORMAT>"), "--format");
        assert_eq!(argument_name("<FINDING>"), "FINDING");
        assert_eq!(argument_name("--quiet"), "--quiet");
        assert_eq!(argument_name("[SUBJECT]..."), "SUBJECT");
    }
}
