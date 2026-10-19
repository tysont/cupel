// ABOUTME: Text rendering of `describe` payloads: the CLI, commands, schemas, languages, facts, checks,
// ABOUTME: finding kinds, and the matrix, laid out as readable sections rather than JSON (ADR-019).
use std::fmt::Write as _;

use cupel_core::{
    AllDescription, ArgDescription, CheckDescription, CliDescription, CommandDescription,
    Description, FactDescription, FindingDescription, LanguageDescription, LanguagesDescription,
    Manifest, MatrixDescription, ProducerDescription, SchemaDescription, Stability,
    SupportDescription, ToolDescription, ToolchainDescription, ValueType,
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
        Description::Language(language) => render_language(language, paint),
        Description::Languages(languages) => render_languages(languages, paint),
        Description::Fact(fact) => render_fact(fact, paint),
        Description::Matrix(matrix) => render_matrix(matrix, paint),
        Description::Check(check) => render_check(check, paint),
        Description::Finding(finding) => render_finding(finding, paint),
        // The guide is Markdown to paste as is.
        Description::Agent(agent) => agent.markdown.clone(),
    }
}

fn render_check(check: &CheckDescription, paint: &Paint) -> String {
    let manifest = &check.manifest;
    let mut out = format!(
        "{} · tier {} · cost {} · version {} · {}\n  {}\n",
        paint.bold(&format!("check {}", manifest.id)),
        manifest.tier.value(),
        name(manifest.cost),
        manifest.version,
        name(manifest.stability),
        manifest.summary
    );
    let languages = check
        .languages
        .iter()
        .map(|l| (l.lang.as_str().to_owned(), support(&l.support)))
        .collect();
    let mut sections = manifest_sections(manifest);
    sections.push(("languages", languages));
    for (heading, rows) in sections {
        section(&mut out, heading, &rows, paint);
    }
    out
}

/// A manifest's sections: facts, findings, metrics, and settings when it has any.
fn manifest_sections(manifest: &Manifest) -> Vec<(&'static str, Vec<(String, String)>)> {
    let kinds = |list: &[cupel_core::FactKind]| {
        let names: Vec<String> = list.iter().map(ToString::to_string).collect();
        none_if_empty(&names.join(", "))
    };
    let facts = vec![
        ("needs".to_owned(), kinds(&manifest.needs)),
        ("wants".to_owned(), kinds(&manifest.wants)),
    ];
    let findings = manifest
        .findings
        .iter()
        .map(|f| {
            (
                f.kind.clone(),
                format!("{} · {}", name(f.action), f.summary),
            )
        })
        .collect();
    let metrics = manifest
        .metrics
        .iter()
        .map(|m| (m.name.clone(), format!("{} · {}", name(m.per), m.summary)))
        .collect();
    let settings: Vec<(String, String)> = manifest
        .settings
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                format!("{} · {}", default(&s.default), s.summary),
            )
        })
        .collect();
    let mut sections = vec![
        ("facts", facts),
        ("findings", findings),
        ("metrics", metrics),
    ];
    // A check without settings has no settings section; every other section is always shown.
    if !settings.is_empty() {
        sections.push(("settings", settings));
    }
    sections
}

/// A setting's default as text: its JSON, or `none` when it has no default.
fn default(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "none".to_owned(),
        value => value.to_string(),
    }
}

/// A blank line, a bold heading, and `rows` as aligned `key  value` lines.
fn section(out: &mut String, heading: &str, rows: &[(String, String)], paint: &Paint) {
    let _ = writeln!(out, "\n{}", paint.bold(heading));
    let width = column(rows.iter().map(|(key, _)| key.len()));
    for (key, value) in rows {
        let _ = writeln!(out, "  {key:<width$}  {value}");
    }
}

fn render_finding(finding: &FindingDescription, paint: &Paint) -> String {
    let body = serde_json::to_string_pretty(&finding.evidence).unwrap_or_default();
    format!(
        "{} · from {} · action {} · {}
  {}

{}
{body}
",
        paint.bold(&format!("finding {}", finding.kind)),
        finding.check,
        name(finding.action),
        name(finding.stability),
        finding.summary,
        paint.bold("evidence")
    )
}

fn render_languages(languages: &LanguagesDescription, paint: &Paint) -> String {
    let mut out = format!("{}\n", paint.bold("languages"));
    let id_width = column(languages.languages.iter().map(|l| l.id.as_str().len()));
    let family_width = column(languages.languages.iter().map(|l| l.family.as_str().len()));
    let ext_width = column(
        languages
            .languages
            .iter()
            .map(|l| l.extensions.join(" ").len()),
    );
    for language in &languages.languages {
        let facts: Vec<String> = language.facts.iter().map(ToString::to_string).collect();
        let _ = writeln!(
            out,
            "  {:<id_width$}  {:<family_width$}  {:<ext_width$}  {} · facts: {}",
            language.id.as_str(),
            language.family.as_str(),
            language.extensions.join(" "),
            name(language.stability),
            none_if_empty(&facts.join(", "))
        );
    }
    out
}

fn render_language(language: &LanguageDescription, paint: &Paint) -> String {
    let mut out = format!(
        "{} · family {} · {} · {}\n",
        paint.bold(&format!("language {}", language.id)),
        language.family,
        language.extensions.join(" "),
        name(language.stability)
    );
    let _ = writeln!(out, "\n{}", paint.bold("facts"));
    if language.facts.is_empty() {
        let _ = writeln!(out, "  none");
    }
    let width = column(language.facts.iter().map(|f| f.kind.as_str().len()));
    for fact in &language.facts {
        let _ = writeln!(
            out,
            "  {:<width$}  {}",
            fact.kind.as_str(),
            producers(&fact.producers)
        );
    }
    let _ = writeln!(out, "\n{}", paint.bold("checks"));
    if language.checks.is_empty() {
        let _ = writeln!(out, "  none registered");
    }
    let width = column(language.checks.iter().map(|c| c.check.as_str().len()));
    for check in &language.checks {
        let _ = writeln!(
            out,
            "  {:<width$}  {}",
            check.check.as_str(),
            support(&check.support)
        );
    }
    if let Some(toolchain) = &language.toolchain {
        render_toolchain(&mut out, toolchain, paint);
    }
    out
}

/// The toolchain row: launchers and tools in default order, each with when it is chosen and
/// its command templates, then the property, fuzz, and race tool names.
fn render_toolchain(out: &mut String, toolchain: &ToolchainDescription, paint: &Paint) {
    let _ = writeln!(out, "\n{}", paint.bold("toolchain"));
    if !toolchain.launchers.is_empty() {
        let _ = writeln!(out, "  launchers");
        for launcher in &toolchain.launchers {
            let values: Vec<String> = launcher
                .values
                .iter()
                .map(|(name, value)| format!("<{name}> = {value}"))
                .collect();
            let _ = writeln!(
                out,
                "    {}  {}",
                launcher.tool,
                when(&launcher.detect, "otherwise")
            );
            let _ = writeln!(out, "      {}", values.join(" · "));
        }
    }
    for (title, tools, fallback) in [
        ("runners", &toolchain.runners, "otherwise"),
        ("coverage", &toolchain.coverage, "always"),
        ("bench", &toolchain.bench, "always"),
    ] {
        if !tools.is_empty() {
            let _ = writeln!(out, "  {title}");
            for tool in tools {
                render_tool(out, tool, fallback);
            }
        }
    }
    for (title, names) in [
        ("property", &toolchain.property),
        ("fuzz", &toolchain.fuzz),
        ("race", &toolchain.race),
    ] {
        let names: Vec<&str> = names.iter().map(cupel_core::ToolId::as_str).collect();
        let _ = writeln!(out, "  {title:<8}  {}", none_if_empty(&names.join(", ")));
    }
}

fn render_tool(out: &mut String, tool: &ToolDescription, fallback: &str) {
    let format = tool
        .format
        .as_ref()
        .map_or_else(String::new, |format| format!("{format} · "));
    let _ = writeln!(
        out,
        "    {}  {format}{}",
        tool.producer,
        when(&tool.detect, fallback)
    );
    for template in &tool.commands {
        match &template.runner {
            Some(runner) => {
                let _ = writeln!(out, "      with {runner}: {}", template.command);
            }
            None => {
                let _ = writeln!(out, "      {}", template.command);
            }
        }
    }
    if let Some(install) = &tool.install {
        let _ = writeln!(out, "      install: {install}");
    }
}

/// When a launcher or tool is chosen: any one of its rules, or `fallback` when it has none.
fn when(rules: &[String], fallback: &str) -> String {
    if rules.is_empty() {
        fallback.to_owned()
    } else {
        format!("when {}", rules.join("; or "))
    }
}

fn render_fact(fact: &FactDescription, paint: &Paint) -> String {
    let version = fact.version.map_or_else(
        || "no payload type yet".to_owned(),
        |v| format!("version {v}"),
    );
    let origin = if fact.syntactic {
        "syntactic"
    } else {
        "execution"
    };
    let mut out = format!(
        "{} · {version} · {origin}\n",
        paint.bold(&format!("fact {}", fact.kind))
    );
    let _ = writeln!(out, "\n{}", paint.bold("producers"));
    let width = column(fact.producers.iter().map(|p| p.lang.as_str().len()));
    for lang in &fact.producers {
        let _ = writeln!(
            out,
            "  {:<width$}  {}",
            lang.lang.as_str(),
            producers(&lang.producers)
        );
    }
    let consumers: Vec<&str> = fact
        .consumers
        .iter()
        .map(cupel_core::CheckId::as_str)
        .collect();
    let _ = writeln!(
        out,
        "\n{}\n  {}",
        paint.bold("consumers"),
        none_if_empty(&consumers.join(", "))
    );
    if let Some(id) = fact
        .schema
        .as_ref()
        .and_then(|schema| schema.get("$id"))
        .and_then(serde_json::Value::as_str)
    {
        let _ = writeln!(out, "\n{}\n  {id}", paint.bold("schema"));
    }
    out
}

fn render_matrix(matrix: &MatrixDescription, paint: &Paint) -> String {
    let mut out = format!(
        "{} · {} checks × {} languages\n",
        paint.bold("matrix"),
        matrix.checks.len(),
        matrix.languages.len()
    );
    let check_width = column(matrix.checks.iter().map(|c| c.as_str().len())).max(5);
    let cells: Vec<Vec<String>> = matrix
        .checks
        .iter()
        .map(|check| {
            matrix
                .languages
                .iter()
                .map(|lang| {
                    matrix
                        .cells
                        .iter()
                        .find(|cell| cell.check == *check && cell.lang == *lang)
                        .map_or_else(String::new, |cell| support(&cell.support))
                })
                .collect()
        })
        .collect();
    let widths: Vec<usize> = matrix
        .languages
        .iter()
        .enumerate()
        .map(|(i, lang)| {
            cells
                .iter()
                .map(|row| row[i].len())
                .chain([lang.as_str().len()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut header = format!("  {:<check_width$}", "check");
    for (lang, width) in matrix.languages.iter().zip(&widths) {
        let _ = write!(header, "  {:<width$}", lang.as_str());
    }
    let _ = writeln!(out, "{}", header.trim_end());
    if matrix.checks.is_empty() {
        let _ = writeln!(out, "  no checks registered");
    }
    for (check, row) in matrix.checks.iter().zip(&cells) {
        let mut line = format!("  {:<check_width$}", check.as_str());
        for (cell, width) in row.iter().zip(&widths) {
            let _ = write!(line, "  {cell:<width$}");
        }
        let _ = writeln!(out, "{}", line.trim_end());
    }
    out.push_str(&verb_rows(matrix, paint));
    out
}

/// Each verb that reads a fact directly, with whether each language can produce it.
fn verb_rows(matrix: &MatrixDescription, paint: &Paint) -> String {
    let mut out = String::new();
    for verb in &matrix.verbs {
        let _ = writeln!(out, "\n{} needs {}", paint.bold(&verb.verb), verb.fact);
        for language in &verb.languages {
            let _ = writeln!(
                out,
                "  {:<12}{}",
                language.lang.as_str(),
                support(&language.support)
            );
        }
    }
    out
}

/// `lang.rust (quick)`, comma-separated, or `none`.
fn producers(list: &[ProducerDescription]) -> String {
    let described: Vec<String> = list
        .iter()
        .map(|producer| {
            let needs: Vec<String> = producer.needs.iter().map(ToString::to_string).collect();
            if needs.is_empty() {
                format!("{} ({})", producer.id, name(producer.cost))
            } else {
                format!(
                    "{} ({}, needs {})",
                    producer.id,
                    name(producer.cost),
                    needs.join(", ")
                )
            }
        })
        .collect();
    none_if_empty(&described.join(", "))
}

/// `supported`, `unsupported(<fact>)`, `unavailable(<tool>)`, or `unconfigured(<key>)`.
fn support(support: &SupportDescription) -> String {
    match support {
        SupportDescription::Supported => "supported".to_owned(),
        SupportDescription::Unsupported { fact } => format!("unsupported({fact})"),
        SupportDescription::Unavailable { tool } => format!("unavailable({tool})"),
        SupportDescription::Unconfigured { key } => format!("unconfigured({key})"),
    }
}

fn none_if_empty(text: &str) -> String {
    if text.is_empty() {
        "none".to_owned()
    } else {
        text.to_owned()
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
        // Each block's unindented lines, up to the first ` · `.
        let headings: Vec<&str> = out
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with(' '))
            .map(|line| line.split(" · ").next().unwrap_or(line))
            .collect();
        assert_eq!(
            headings,
            [
                "cupel [version]",
                "verbs",
                "global flags",
                "flags without a verb",
                "exit codes",
                "schemas",
                "configuration precedence",
                "cupel init",
                "cupel doctor",
                "cupel describe",
                "cupel check",
                "cupel survey",
                "cupel explain",
                "cupel baseline",
                "cupel oracle",
                "cupel tests",
                "cupel suppress",
                "cupel serve",
                "schema tiny",
                "{}"
            ]
        );
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
