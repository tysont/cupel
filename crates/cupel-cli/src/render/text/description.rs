// ABOUTME: Text rendering of `describe` payloads: the CLI, one command with its sub-verbs, a schema,
// ABOUTME: or all of them, laid out as readable sections rather than JSON (ADR-019).
use std::fmt::Write as _;

use cupel_core::{
    AllDescription, ArgDescription, CliDescription, CommandDescription, Description,
    SchemaDescription, Stability, ValueType,
};

use super::{Paint, continued, name};

/// The widest name column; longer names push their summary onto the next line.
const NAME_COLUMN: usize = 28;

pub fn render(description: &Description, paint: &Paint) -> String {
    match description {
        Description::Cli(cli) => render_cli(cli, paint),
        Description::Command(command) => {
            let mut out = String::new();
            render_command(&mut out, command, 0, None, paint);
            out
        }
        Description::Schema(schema) => render_schema(schema, None, paint),
        Description::All(all) => render_all(all, paint),
    }
}

fn render_all(all: &AllDescription, paint: &Paint) -> String {
    let mut blocks = vec![render_cli(&all.cli, paint)];
    for command in &all.commands {
        let mut out = String::new();
        render_command(&mut out, command, 0, Some(all.cli.stability), paint);
        blocks.push(out);
    }
    for schema in &all.schemas {
        blocks.push(render_schema(schema, Some(all.cli.stability), paint));
    }
    blocks.join("\n")
}

/// ` · <stability>` when it differs from `reference`, or always when there is none.
fn stability(own: Stability, reference: Option<Stability>) -> String {
    if reference == Some(own) {
        String::new()
    } else {
        format!(" · {}", name(own))
    }
}

fn pending(step: Option<&str>) -> String {
    step.map(|step| format!(" · pending {step}"))
        .unwrap_or_default()
}

fn render_cli(cli: &CliDescription, paint: &Paint) -> String {
    let reference = Some(cli.stability);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} · {}{}",
        paint.bold(&format!("{} {}", cli.name, cli.version)),
        cli.summary,
        stability(cli.stability, None)
    );
    let _ = writeln!(out, "\n{}", paint.bold("verbs"));
    let width = column(cli.verbs.iter().map(|verb| verb.name.len()));
    for verb in &cli.verbs {
        let _ = writeln!(
            out,
            "  {:<width$}  {}{}{}",
            verb.name,
            verb.summary,
            stability(verb.stability, reference),
            pending(verb.pending_step.as_deref())
        );
    }
    let _ = writeln!(out, "\n{}", paint.bold("global flags"));
    arguments(&mut out, &cli.global_flags, 2, reference);
    if !cli.flags.is_empty() {
        let _ = writeln!(out, "\n{}", paint.bold("flags without a verb"));
        arguments(&mut out, &cli.flags, 2, reference);
    }
    let _ = writeln!(out, "\n{}", paint.bold("exit codes"));
    let width = column(cli.exit_codes.iter().map(|code| code.name.len()));
    for code in &cli.exit_codes {
        let _ = writeln!(
            out,
            "  {}  {:<width$}  {}{}",
            code.code.code(),
            code.name,
            code.meaning,
            stability(code.stability, reference)
        );
    }
    let _ = writeln!(out, "\n{}", paint.bold("schemas"));
    let width = column(cli.schemas.iter().map(|schema| schema.name.len()));
    for schema in &cli.schemas {
        let _ = writeln!(
            out,
            "  {:<width$}  {}{}",
            schema.name,
            schema.url,
            stability(schema.stability, reference)
        );
    }
    let precedence: Vec<String> = cli.config_precedence.iter().map(|s| name(*s)).collect();
    let _ = writeln!(
        out,
        "\n{}\n  {}",
        paint.bold("configuration precedence"),
        precedence.join(" > ")
    );
    out
}

/// A command at `indent`, then its arguments, examples, and sub-verbs (each one level deeper).
/// `reference` is the stability of the enclosing description, if any.
fn render_command(
    out: &mut String,
    command: &CommandDescription,
    indent: usize,
    reference: Option<Stability>,
    paint: &Paint,
) {
    let pad = " ".repeat(indent);
    let _ = writeln!(
        out,
        "{pad}{} · {}{}{}",
        paint.bold(&format!("cupel {}", command.name)),
        continued(&command.summary, indent + 2),
        stability(command.stability, reference),
        pending(command.pending_step.as_deref())
    );
    let own = Some(command.stability);
    if !command.arguments.is_empty() {
        let _ = writeln!(out, "{pad}  {}", paint.bold("arguments"));
        arguments(out, &command.arguments, indent + 4, own);
    }
    if !command.examples.is_empty() {
        let _ = writeln!(out, "{pad}  {}", paint.bold("examples"));
        for example in &command.examples {
            let _ = writeln!(
                out,
                "{pad}    $ {}  {}",
                example.command,
                paint.dim(&format!(
                    "# {} (exit {})",
                    example.summary,
                    example.exit_code.code()
                ))
            );
        }
    }
    if !command.subcommands.is_empty() {
        let _ = writeln!(out, "{pad}  {}", paint.bold("sub-verbs"));
        for sub in &command.subcommands {
            render_command(out, sub, indent + 4, own, paint);
        }
    }
}

/// One row per argument: its spelling and summary, then a line of facts when it has any.
fn arguments(
    out: &mut String,
    list: &[ArgDescription],
    indent: usize,
    reference: Option<Stability>,
) {
    let spellings: Vec<String> = list.iter().map(spelling).collect();
    let width = column(spellings.iter().map(String::len));
    let pad = " ".repeat(indent);
    let facts_pad = " ".repeat(indent + width + 2);
    for (arg, spelling) in list.iter().zip(&spellings) {
        if spelling.len() > width {
            let _ = writeln!(out, "{pad}{spelling}\n{facts_pad}{}", arg.summary);
        } else {
            let _ = writeln!(out, "{pad}{spelling:<width$}  {}", arg.summary);
        }
        let facts = facts(arg, reference);
        if !facts.is_empty() {
            let _ = writeln!(out, "{facts_pad}{}", facts.join(" · "));
        }
    }
}

/// `--flag VALUE`, `--switch`, or `<POSITIONAL>`.
fn spelling(arg: &ArgDescription) -> String {
    match (&arg.value_name, arg.positional) {
        (Some(value), false) => format!("{} {value}", arg.name),
        (_, true) => format!("<{}>", arg.name),
        (None, false) => arg.name.clone(),
    }
}

fn facts(arg: &ArgDescription, reference: Option<Stability>) -> Vec<String> {
    let mut facts = Vec::new();
    if matches!(
        arg.value,
        ValueType::Integer | ValueType::Path | ValueType::Duration
    ) {
        facts.push(name(arg.value));
    }
    if !arg.values.is_empty() {
        facts.push(format!("one of {}", arg.values.join(", ")));
    }
    if let Some(default) = &arg.default {
        facts.push(format!("default {default}"));
    }
    if let Some(env) = &arg.env {
        facts.push(format!("env {env}"));
    }
    if arg.required {
        facts.push("required".to_owned());
    }
    if arg.repeated {
        facts.push("repeatable".to_owned());
    }
    if !arg.conflicts.is_empty() {
        facts.push(format!("conflicts with {}", arg.conflicts.join(", ")));
    }
    if reference != Some(arg.stability) {
        facts.push(name(arg.stability));
    }
    if let Some(step) = &arg.pending_step {
        facts.push(format!("pending {step}"));
    }
    facts
}

fn render_schema(
    schema: &SchemaDescription,
    reference: Option<Stability>,
    paint: &Paint,
) -> String {
    let body = serde_json::to_string_pretty(&schema.schema).unwrap_or_default();
    format!(
        "{} · {}{}\n{body}\n",
        paint.bold(&format!("schema {}", schema.name)),
        schema.url,
        stability(schema.stability, reference)
    )
}

/// The name column's width: the widest name, capped at `NAME_COLUMN`.
fn column(widths: impl Iterator<Item = usize>) -> usize {
    widths.max().unwrap_or(0).min(NAME_COLUMN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry;

    fn plain(description: &Description) -> String {
        render(description, &Paint { color: false })
    }

    fn cli() -> CliDescription {
        CliDescription {
            version: "[version]".to_owned(),
            ..registry::cli()
        }
    }

    fn tiny_schema(schema: &serde_json::Value) -> SchemaDescription {
        SchemaDescription {
            name: "tiny".to_owned(),
            url: "https://cupel.dev/schema/v1/tiny.json".to_owned(),
            stability: Stability::Experimental,
            schema: schema.as_object().cloned().expect("an object"),
        }
    }

    #[test]
    fn the_cli_reads_as_sections_of_verbs_flags_exit_codes_and_schemas() {
        insta::assert_snapshot!(plain(&Description::Cli(cli())));
    }

    #[test]
    fn a_command_shows_its_arguments_examples_and_nested_sub_verbs() {
        let words = ["baseline".to_owned()];
        let baseline = registry::find(&words).expect("baseline is registered");
        insta::assert_snapshot!(plain(&Description::Command(baseline)));
    }

    #[test]
    fn a_schema_is_its_header_then_the_indented_schema() {
        let schema = tiny_schema(&serde_json::json!({"type": "object", "required": ["a"]}));
        assert_eq!(
            plain(&Description::Schema(schema)),
            "schema tiny · https://cupel.dev/schema/v1/tiny.json · experimental\n{\n  \
             \"required\": [\n    \"a\"\n  ],\n  \"type\": \"object\"\n}\n"
        );
    }

    #[test]
    fn everything_is_the_cli_then_every_command_then_every_schema() {
        let all = AllDescription {
            cli: cli(),
            commands: registry::commands(),
            schemas: vec![tiny_schema(&serde_json::json!({}))],
        };
        let out = plain(&Description::All(all));
        assert!(out.starts_with(&plain(&Description::Cli(cli()))), "{out}");
        for command in registry::commands() {
            let mut expected = String::new();
            render_command(
                &mut expected,
                &command,
                0,
                Some(Stability::Experimental),
                &Paint { color: false },
            );
            assert!(out.contains(&expected), "{}", command.name);
        }
        assert!(out.ends_with("schema tiny · https://cupel.dev/schema/v1/tiny.json\n{}\n"));
    }

    #[test]
    fn argument_facts_name_everything_but_the_obvious() {
        let arg = registry::cli()
            .global_flags
            .into_iter()
            .find(|arg| arg.name == "--format")
            .expect("--format is registered");
        assert_eq!(
            facts(&arg, Some(Stability::Experimental)),
            ["one of json, jsonl, text, sarif", "env CUPEL_FORMAT"]
        );
        assert!(facts(&arg, Some(Stability::Stable)).contains(&"experimental".to_owned()));
    }
}
