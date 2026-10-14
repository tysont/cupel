// ABOUTME: `CupelError`, the failure every verb can return, and its mapping to an exit code and a
// ABOUTME: `cli.*` finding: usage errors and unbuilt features exit 2, internal errors exit 6.
use clap::error::{ContextKind, ContextValue, ErrorKind};
use cupel_core::config::ConfigError;
use cupel_core::{ActionKind, ExitCode, Finding, Severity};
use serde_json::json;

use crate::problem::Problem;

/// Why a verb did not produce a result. See ADR-018 for the findings each variant becomes.
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
        let problem = match self {
            Self::Usage(usage) => usage.problem("cli", "cli.usage"),
            Self::NotImplemented(pending) => Problem {
                check: "cli",
                kind: "cli.not-implemented",
                subject: pending.subject.clone(),
                severity: Severity::Fail,
                title: self.to_string(),
                detail: Some(format!("Implemented in plan step {}.", pending.step)),
                evidence: json!({"step": pending.step}),
                action: ActionKind::Review,
                fix: pending.fix.clone(),
            },
            Self::Internal(report) => Problem {
                check: "cli",
                kind: "cli.internal",
                subject: report.location.clone(),
                severity: Severity::Fail,
                title: format!("Cupel failed internally: {}", report.message),
                detail: Some(
                    "This is a bug in Cupel, not in the code under test. Please report it with \
                     the bug_report evidence."
                        .to_owned(),
                ),
                evidence: json!({"bug_report": {
                    "version": env!("CARGO_PKG_VERSION"),
                    "os": std::env::consts::OS,
                    "arch": std::env::consts::ARCH,
                    "message": report.message,
                    "location": report.location,
                    "args": report.args,
                    "report": concat!(env!("CARGO_PKG_REPOSITORY"), "/issues/new"),
                }}),
                action: ActionKind::Review,
                fix: "Report this Cupel bug with the bug_report evidence attached.".to_owned(),
            },
        };
        problem.finding(repro)
    }
}

impl From<cupel_core::Error> for CupelError {
    /// Core errors reach the CLI from configuration and from scope and selection flags; every
    /// one is a usage error (ADR-017, ADR-040).
    fn from(error: cupel_core::Error) -> Self {
        Usage::from_core(&error).into()
    }
}

impl From<Usage> for CupelError {
    fn from(usage: Usage) -> Self {
        Self::Usage(Box::new(usage))
    }
}

impl Usage {
    /// Describes a core error, which reaches the CLI from configuration or a scope or selection
    /// flag.
    pub fn from_core(error: &cupel_core::Error) -> Self {
        match error {
            cupel_core::Error::Config(config) => Self::from_config(config),
            cupel_core::Error::Argument(argument) => Self {
                subject: argument.argument.clone(),
                title: argument.title.clone(),
                fix: argument.fix.clone(),
                detail: None,
                argument: Some(argument.argument.clone()),
                value: argument.value.clone(),
                valid: argument.valid.clone(),
            },
            other => Self {
                subject: "configuration".to_owned(),
                title: other.to_string(),
                fix: "Fix the configuration; `cupel describe schema config` gives its shape."
                    .to_owned(),
                detail: None,
                argument: None,
                value: None,
                valid: Vec::new(),
            },
        }
    }

    /// This usage problem as `check`'s finding of `kind`, asking to configure: its evidence
    /// names the argument, the value given, and the valid values.
    pub fn problem(&self, check: &'static str, kind: &'static str) -> Problem {
        Problem {
            check,
            kind,
            subject: self.subject.clone(),
            severity: Severity::Fail,
            title: self.title.clone(),
            detail: self.detail.clone(),
            evidence: json!({"argument": self.argument, "value": self.value, "valid": self.valid}),
            action: ActionKind::Configure,
            fix: self.fix.clone(),
        }
    }

    /// Describes a configuration problem: which file, variable, or flag is at fault, and the fix.
    /// The subject is the flag, variable, file, or key the user edits (ADR-016).
    pub fn from_config(error: &ConfigError) -> Self {
        let title = error.to_string();
        let usage = |subject: &str, fix: String| Self {
            subject: subject.to_owned(),
            title: title.clone(),
            fix,
            detail: None,
            argument: None,
            value: None,
            valid: Vec::new(),
        };
        match error {
            ConfigError::Invalid { path, message } => {
                // toml ends its message with the problem itself, such as "unknown field `x`,
                // expected one of …"; the lines before it locate the problem in the file.
                let problem = message.lines().last().unwrap_or(message).trim();
                let short = problem.split(", expected").next().unwrap_or(problem);
                Self {
                    title: format!("Invalid configuration in `{}`: {short}", path.display()),
                    detail: Some(title.clone()),
                    value: Some(path.display().to_string()),
                    ..usage(
                        "cupel.toml",
                        format!("Edit `{}`: {problem}.", path.display()),
                    )
                }
            }
            ConfigError::Missing { path } => Self {
                argument: Some("--config".to_owned()),
                value: Some(path.display().to_string()),
                ..usage(
                    "--config",
                    "Pass --config an existing file, or omit it to use the nearest cupel.toml."
                        .to_owned(),
                )
            },
            ConfigError::Unreadable { path, .. } => Self {
                value: Some(path.display().to_string()),
                ..usage(
                    "cupel.toml",
                    format!(
                        "Make `{}` readable, or pass --config another file.",
                        path.display()
                    ),
                )
            },
            ConfigError::InvalidEnv { var, value, reason } => Self {
                argument: Some((*var).to_owned()),
                value: Some(value.clone()),
                ..usage(
                    var,
                    format!("Set {var} to a valid value or unset it: {reason}."),
                )
            },
            ConfigError::UnknownProfile { name, defined, .. } => Self {
                argument: Some("--profile".to_owned()),
                value: Some(name.clone()),
                valid: defined.clone(),
                ..usage(
                    "--profile",
                    if defined.is_empty() {
                        "Define [profile.NAME] in cupel.toml, or select no profile.".to_owned()
                    } else {
                        format!("Select one of: {}.", defined.join(", "))
                    },
                )
            },
            ConfigError::CheckSetting {
                check, key, valid, ..
            } => Self {
                value: Some(key.clone()),
                valid: valid.clone(),
                ..usage(
                    &format!("checks.{check}.{key}"),
                    format!(
                        "Fix `{key}` under [checks.\"{check}\"] in cupel.toml (or its per-language \
                         overrides); valid settings: {}.",
                        valid.join(", ")
                    ),
                )
            },
            ConfigError::InvalidGateRule { key, .. } => Self {
                value: Some(key.clone()),
                ..usage(
                    key,
                    "Write gate rules as \"<check-id>[.<metric>]\" = \"<op> <number>\" with op \
                     one of >= > <= < ==, or \"<check-id>\" = \"none\" | \"no-breaking\"."
                        .to_owned(),
                )
            },
            ConfigError::Unregistered {
                section,
                registered,
                ..
            } => Self {
                valid: registered.clone(),
                ..usage(section, format!("Use a registered ID in `{section}`."))
            },
        }
    }

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
    use cupel_core::{CheckId, FindingId};
    use serde_json::Value;

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
