// ABOUTME: Chooses the output format: the configured one (`--format` > `CUPEL_FORMAT` > profile > file),
// ABOUTME: else JSON for a pipe and text for a terminal; also recovers the choice when parsing failed.
use std::ffi::OsString;

use cupel_core::config::Format;

use crate::cli::format_named;
use crate::error::CupelError;

/// The renderers that exist. Formats without one are reported as not implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rendering {
    Json,
    Jsonl,
    Text,
}

/// The format used when no configuration layer chose one.
pub fn default_for(stdout_is_terminal: bool) -> Format {
    if stdout_is_terminal {
        Format::Text
    } else {
        Format::Json
    }
}

/// The renderer for `format`, or the plan step that builds it.
pub fn rendering(format: Format) -> Result<Rendering, CupelError> {
    match format {
        Format::Json => Ok(Rendering::Json),
        Format::Text => Ok(Rendering::Text),
        Format::Jsonl => Ok(Rendering::Jsonl),
        Format::Sarif => Err(CupelError::pending_format(format.as_str(), "P6.1")),
    }
}

/// What can be read from a command line clap rejected (ADR-018 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recovered {
    pub format: Format,
    pub quiet: bool,
    pub describe: bool,
    pub no_color: bool,
}

/// Scans the arguments before any `--` for `--format`, `--quiet`, `--describe`, and `--no-color`. The format
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
        .and_then(format_named)
        .unwrap_or_else(|| default_for(stdout_is_terminal));
    Recovered {
        format,
        quiet: args.iter().any(|arg| arg == "--quiet"),
        describe: args.iter().any(|arg| arg == "--describe"),
        no_color: args.iter().any(|arg| arg == "--no-color"),
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
        assert_eq!(default_for(false), Format::Json);
        assert_eq!(default_for(true), Format::Text);
    }

    #[test]
    fn json_jsonl_and_text_render_and_sarif_names_its_step() {
        assert_eq!(rendering(Format::Json), Ok(Rendering::Json));
        assert_eq!(rendering(Format::Jsonl), Ok(Rendering::Jsonl));
        assert_eq!(rendering(Format::Text), Ok(Rendering::Text));
        match rendering(Format::Sarif) {
            Err(CupelError::NotImplemented(pending)) => assert_eq!(pending.step, "P6.1"),
            other => panic!("sarif: {other:?}"),
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
    fn recover_reads_quiet_describe_and_no_color_but_not_after_a_double_dash() {
        let found = recover(
            &args(&["--quiet", "x", "--describe", "--no-color"]),
            None,
            false,
        );
        assert!(found.quiet && found.describe && found.no_color);
        let after = recover(
            &args(&["x", "--", "--quiet", "--describe", "--no-color"]),
            None,
            false,
        );
        assert!(!after.quiet && !after.describe && !after.no_color);
    }
}
