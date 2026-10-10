// ABOUTME: Turns an envelope into stdout text: the complete JSON envelope, or a minimal text view.
// ABOUTME: The text view is a stand-in until the full renderer of P0.5 replaces this module.
use std::fmt::Write as _;

use cupel_core::{
    ArgDescription, CliDescription, CommandDescription, Description, Envelope, Finding, Status,
};

use crate::cli::verb_name;
use crate::format::Rendering;

pub fn render(envelope: &Envelope, rendering: Rendering) -> String {
    match rendering {
        Rendering::Json => json(envelope),
        Rendering::Text => text(envelope),
    }
}

fn json(envelope: &Envelope) -> String {
    let mut out = serde_json::to_string_pretty(envelope).expect("envelopes serialize");
    out.push('\n');
    out
}

/// Status line, then the description if any, then each finding with its fix.
fn text(envelope: &Envelope) -> String {
    let mut out = String::new();
    let verb = envelope
        .run
        .verb
        .map(|verb| format!(" {}", verb_name(verb)))
        .unwrap_or_default();
    let _ = writeln!(
        out,
        "cupel{verb}: {} (exit {})",
        status_name(envelope.status),
        envelope.exit_code.code()
    );
    if let Some(description) = &envelope.description {
        describe(&mut out, description);
    }
    for finding in &envelope.findings {
        finding_lines(&mut out, finding);
    }
    out
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Pass => "pass",
        Status::Fail => "fail",
        Status::Partial => "partial",
        Status::Error => "error",
        Status::Plan => "plan",
    }
}

fn finding_lines(out: &mut String, finding: &Finding) {
    let severity = serde_json::to_value(finding.severity).unwrap_or_default();
    let action = serde_json::to_value(finding.action.kind).unwrap_or_default();
    let _ = writeln!(
        out,
        "{} {} {}: {}",
        severity.as_str().unwrap_or_default(),
        finding.kind,
        finding.id,
        finding.title
    );
    if let Some(detail) = &finding.detail {
        let _ = writeln!(out, "  detail: {}", detail.replace('\n', "\n    "));
    }
    let _ = writeln!(
        out,
        "  action: {} — {}",
        action.as_str().unwrap_or_default(),
        finding.action.summary
    );
}

fn describe(out: &mut String, description: &Description) {
    match description {
        Description::Cli(cli) => describe_cli(out, cli),
        Description::Command(command) => describe_command(out, command),
        Description::Schema(schema) => {
            let _ = writeln!(out, "{} {}", schema.name, schema.url);
            let _ = writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&schema.schema).unwrap_or_default()
            );
        }
        Description::All(all) => {
            let _ = writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(all).unwrap_or_default()
            );
        }
    }
}

fn describe_cli(out: &mut String, cli: &CliDescription) {
    let _ = writeln!(out, "{}\n\nVerbs:", cli.summary);
    for verb in &cli.verbs {
        let _ = writeln!(
            out,
            "  {:<10} {}{}",
            verb.name,
            verb.summary,
            pending(verb.pending_step.as_deref())
        );
    }
    let _ = writeln!(out, "Global flags:");
    arguments(out, &cli.global_flags);
    let _ = writeln!(out, "Exit codes:");
    for code in &cli.exit_codes {
        let _ = writeln!(
            out,
            "  {} {:<12} {}",
            code.code.code(),
            code.name,
            code.meaning
        );
    }
    let _ = writeln!(out, "Schemas:");
    for schema in &cli.schemas {
        let _ = writeln!(out, "  {:<8} {}", schema.name, schema.url);
    }
}

fn describe_command(out: &mut String, command: &CommandDescription) {
    let _ = writeln!(
        out,
        "cupel {}: {}{}",
        command.name,
        command.summary,
        pending(command.pending_step.as_deref())
    );
    if !command.arguments.is_empty() {
        let _ = writeln!(out, "Arguments:");
        arguments(out, &command.arguments);
    }
    if !command.subcommands.is_empty() {
        let _ = writeln!(out, "Sub-verbs:");
        for sub in &command.subcommands {
            let _ = writeln!(out, "  {:<16} {}", sub.name, sub.summary);
        }
    }
    let _ = writeln!(out, "Examples:");
    for example in &command.examples {
        let _ = writeln!(
            out,
            "  {}  (exit {})",
            example.command,
            example.exit_code.code()
        );
    }
}

fn arguments(out: &mut String, list: &[ArgDescription]) {
    for arg in list {
        let name = match &arg.value_name {
            Some(value) if !arg.positional => format!("{} {value}", arg.name),
            _ => arg.name.clone(),
        };
        let _ = writeln!(
            out,
            "  {name:<28} {}{}",
            arg.summary,
            pending(arg.pending_step.as_deref())
        );
    }
}

fn pending(step: Option<&str>) -> String {
    step.map(|step| format!(" [pending {step}]"))
        .unwrap_or_default()
}
