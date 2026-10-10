// ABOUTME: Chooses the output format: `--format`, then `CUPEL_FORMAT`, then JSON for a pipe and text
// ABOUTME: for a terminal; also recovers the choice from raw arguments when parsing failed.
use std::ffi::OsString;

use clap::ValueEnum;

use crate::cli::Format;
use crate::error::CupelError;

/// The renderers that exist. Formats without one are reported as not implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rendering {
    Json,
    Text,
}

impl Format {
    /// The format used when neither `--format` nor `CUPEL_FORMAT` chose one.
    pub fn default_for(stdout_is_terminal: bool) -> Self {
        if stdout_is_terminal {
            Self::Text
        } else {
            Self::Json
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Jsonl => "jsonl",
            Self::Text => "text",
            Self::Sarif => "sarif",
        }
    }

    /// The renderer for this format, or the plan step that builds it.
    pub fn rendering(self) -> Result<Rendering, CupelError> {
        match self {
            Self::Json => Ok(Rendering::Json),
            Self::Text => Ok(Rendering::Text),
            Self::Jsonl => Err(CupelError::pending_format(self.name(), "P0.5")),
            Self::Sarif => Err(CupelError::pending_format(self.name(), "P6.1")),
        }
    }
}

/// What can be read from a command line clap rejected (ADR-017 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recovered {
    pub format: Format,
    pub quiet: bool,
    pub describe: bool,
}

/// Scans the arguments before any `--` for `--format`, `--quiet`, and `--describe`. The format
/// falls back to `CUPEL_FORMAT` and then the terminal rule; a source naming an invalid format
/// falls straight back to the terminal rule.
pub fn recover(args: &[OsString], variable: Option<&str>, stdout_is_terminal: bool) -> Recovered {
    let args: Vec<String> = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .take_while(|arg| arg != "--")
        .collect();
    let flag = args.iter().enumerate().find_map(|(index, arg)| {
        if arg == "--format" {
            Some(args.get(index + 1).cloned().unwrap_or_default())
        } else {
            arg.strip_prefix("--format=").map(str::to_owned)
        }
    });
    let format = flag
        .as_deref()
        .or(variable)
        .and_then(|name| Format::from_str(name, false).ok())
        .unwrap_or_else(|| Format::default_for(stdout_is_terminal));
    Recovered {
        format,
        quiet: args.iter().any(|arg| arg == "--quiet"),
        describe: args.iter().any(|arg| arg == "--describe"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn default_is_json_for_a_pipe_and_text_for_a_terminal() {
        assert_eq!(Format::default_for(false), Format::Json);
        assert_eq!(Format::default_for(true), Format::Text);
    }

    #[test]
    fn json_and_text_render_and_the_rest_name_their_step() {
        assert_eq!(Format::Json.rendering(), Ok(Rendering::Json));
        assert_eq!(Format::Text.rendering(), Ok(Rendering::Text));
        for (format, step) in [(Format::Jsonl, "P0.5"), (Format::Sarif, "P6.1")] {
            match format.rendering() {
                Err(CupelError::NotImplemented(pending)) => assert_eq!(pending.step, step),
                other => panic!("{format:?}: {other:?}"),
            }
        }
    }

    #[test]
    fn recover_prefers_the_flag_then_the_variable_then_the_terminal() {
        let pipe = false;
        assert_eq!(
            recover(&args(&["x", "--format", "text"]), Some("json"), pipe).format,
            Format::Text
        );
        assert_eq!(
            recover(&args(&["--format=text", "x"]), None, pipe).format,
            Format::Text
        );
        assert_eq!(
            recover(&args(&["x"]), Some("text"), pipe).format,
            Format::Text
        );
        assert_eq!(recover(&args(&["x"]), None, pipe).format, Format::Json);
        assert_eq!(recover(&args(&["x"]), None, true).format, Format::Text);
    }

    #[test]
    fn recover_falls_back_to_the_terminal_rule_for_an_invalid_format() {
        assert_eq!(
            recover(&args(&["--format", "xml"]), Some("text"), false).format,
            Format::Json
        );
        assert_eq!(
            recover(&args(&["--format"]), None, true).format,
            Format::Text
        );
    }

    #[test]
    fn recover_reads_quiet_and_describe_but_not_after_a_double_dash() {
        let found = recover(&args(&["--quiet", "x", "--describe"]), None, false);
        assert!(found.quiet && found.describe);
        let after = recover(&args(&["x", "--", "--quiet", "--describe"]), None, false);
        assert!(!after.quiet && !after.describe);
    }
}
