// ABOUTME: The `describe` verb: renders the registry for the CLI, one command, a schema, or all of it.
// ABOUTME: Languages, facts, checks, finding kinds, and the matrix come from the registries.
use std::path::Path;

use cupel_core::{
    AgentDescription, AllDescription, CheckId, Description, Envelope, FactKind, LangId,
    SchemaDescription, schema,
};
use serde_json::Value;

use crate::agent;
use crate::catalog::Catalog;
use crate::cli::DescribeArgs;
use crate::error::{CupelError, Usage};
use crate::registry;
use crate::run::Run;

/// `describe` subjects other than verbs; offered when a subject is not recognized.
const SUBJECTS: [&str; 6] = [
    "schema",
    "finding",
    "language",
    "languages",
    "fact",
    "matrix",
];

/// `root` is the repository root, against which tool availability is judged for the matrix.
/// `catalog` holds the registries, plugins included (ADR-088 §7).
pub fn run(
    run: &Run,
    args: &DescribeArgs,
    catalog: &Catalog,
    root: &Path,
) -> Result<Envelope, CupelError> {
    Ok(run.passed(Some(describe(args, catalog, root)?)))
}

/// `<command> --describe`: the command named by `words`, or the CLI when there are none.
pub fn command(run: &Run, words: &[String]) -> Result<Envelope, CupelError> {
    let description = if words.is_empty() {
        Description::Cli(registry::cli())
    } else {
        Description::Command(
            registry::find(words).ok_or_else(|| unknown_subject(&words.join(" ")))?,
        )
    };
    Ok(run.passed(Some(description)))
}

fn describe(
    args: &DescribeArgs,
    catalog: &Catalog,
    root: &Path,
) -> Result<Description, CupelError> {
    if args.agent {
        return Ok(Description::Agent(AgentDescription {
            markdown: agent::guide(catalog.checks()),
        }));
    }
    if args.all {
        return Ok(Description::All(AllDescription {
            cli: registry::cli(),
            commands: registry::commands(),
            schemas: schema::PUBLISHED
                .iter()
                .map(|(name, _)| schema_description(name))
                .collect::<Result<_, _>>()?,
        }));
    }
    let words: Vec<&str> = args.subject.iter().map(String::as_str).collect();
    match words.as_slice() {
        [] => Ok(Description::Cli(registry::cli())),
        ["schema"] => Err(unknown_schema(None)),
        ["schema", name] => schema_description(name).map(Description::Schema),
        ["languages"] => Ok(Description::Languages(catalog.languages())),
        ["matrix"] => Ok(Description::Matrix(catalog.matrix(root))),
        ["language"] => Err(unknown_language(catalog, None)),
        ["language", id] => catalog
            .language(&LangId::from(*id), root)
            .map(Description::Language)
            .ok_or_else(|| unknown_language(catalog, Some(id))),
        ["fact"] => Err(unknown_fact(None)),
        ["fact", kind] => fact_kind(kind)
            .map(|kind| Description::Fact(catalog.fact(kind)))
            .ok_or_else(|| unknown_fact(Some(kind))),
        ["finding"] => Err(unknown_finding(catalog, None)),
        ["finding", kind] => catalog
            .finding(kind)
            .map(Description::Finding)
            .ok_or_else(|| unknown_finding(catalog, Some(kind))),
        [check] if check.contains('.') => catalog
            .check(&CheckId::from(*check), root)
            .map(Description::Check)
            .ok_or_else(|| unknown_check(catalog, check)),
        _ => registry::find(&args.subject)
            .map(Description::Command)
            .ok_or_else(|| unknown_subject(&words.join(" "))),
    }
}

fn schema_description(name: &str) -> Result<SchemaDescription, CupelError> {
    let (_, generate) = schema::PUBLISHED
        .iter()
        .find(|(published, _)| *published == name)
        .ok_or_else(|| unknown_schema(Some(name)))?;
    let reference = registry::schemas()
        .into_iter()
        .find(|reference| reference.name == name)
        .ok_or_else(|| unknown_schema(Some(name)))?;
    let Value::Object(body) = generate().to_value() else {
        unreachable!("a root JSON Schema is an object")
    };
    Ok(SchemaDescription {
        name: reference.name,
        url: reference.url,
        stability: reference.stability,
        schema: body,
    })
}

fn unknown_schema(name: Option<&str>) -> CupelError {
    let valid: Vec<String> = schema::PUBLISHED
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect();
    CupelError::from(Usage {
        subject: "describe schema".to_owned(),
        title: match name {
            Some(name) => format!("No schema named `{name}`"),
            None => "`describe schema` needs a schema name".to_owned(),
        },
        fix: format!("Name a schema: {}.", valid.join(", ")),
        detail: None,
        argument: Some("SUBJECT".to_owned()),
        value: name.map(str::to_owned),
        valid,
    })
}

fn fact_kind(name: &str) -> Option<FactKind> {
    FactKind::all()
        .iter()
        .copied()
        .find(|kind| kind.as_str() == name)
}

fn unknown_language(catalog: &Catalog, id: Option<&str>) -> CupelError {
    let valid: Vec<String> = catalog
        .language_ids()
        .iter()
        .map(ToString::to_string)
        .collect();
    unknown_named("describe language", "language", id, valid)
}

fn unknown_check(catalog: &Catalog, id: &str) -> CupelError {
    let valid: Vec<String> = catalog
        .checks()
        .iter()
        .map(|check| check.manifest().id.to_string())
        .collect();
    unknown_named("describe CHECK", "check", Some(id), valid)
}

fn unknown_finding(catalog: &Catalog, kind: Option<&str>) -> CupelError {
    let valid = catalog.finding_kinds();
    unknown_named("describe finding", "finding kind", kind, valid)
}

fn unknown_fact(kind: Option<&str>) -> CupelError {
    let valid: Vec<String> = FactKind::all().iter().map(ToString::to_string).collect();
    unknown_named("describe fact", "fact kind", kind, valid)
}

/// A usage error for `describe <subject> <name>` where `name` is missing or not one of `valid`.
fn unknown_named(subject: &str, noun: &str, name: Option<&str>, valid: Vec<String>) -> CupelError {
    CupelError::from(Usage {
        subject: subject.to_owned(),
        title: match name {
            Some(name) => format!("No {noun} named `{name}`"),
            None => format!("`{subject}` needs a {noun}"),
        },
        fix: format!("Name a {noun}: {}.", valid.join(", ")),
        detail: None,
        argument: Some("SUBJECT".to_owned()),
        value: name.map(str::to_owned),
        valid,
    })
}

fn unknown_subject(subject: &str) -> CupelError {
    let valid: Vec<String> = registry::commands()
        .into_iter()
        .map(|command| command.name)
        .chain(SUBJECTS.iter().map(|s| (*s).to_owned()))
        .collect();
    CupelError::from(Usage {
        subject: "describe".to_owned(),
        title: format!("Nothing to describe called `{subject}`"),
        fix: format!("Describe one of: {}.", valid.join(", ")),
        detail: None,
        argument: Some("SUBJECT".to_owned()),
        value: Some(subject.to_owned()),
        valid,
    })
}
