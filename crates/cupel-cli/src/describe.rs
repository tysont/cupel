// ABOUTME: The `describe` verb: renders the registry for the CLI, one command, a schema, or all of it.
// ABOUTME: Subjects whose data arrives in later steps (matrix, checks, languages) report that step.
use cupel_core::{AllDescription, Description, Envelope, SchemaDescription, schema};
use serde_json::Value;

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

pub fn run(run: &Run, args: &DescribeArgs) -> Result<Envelope, CupelError> {
    Ok(run.passed(Some(describe(args)?)))
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

fn describe(args: &DescribeArgs) -> Result<Description, CupelError> {
    if args.agent {
        return Err(CupelError::pending_description("describe --agent", "P8.3"));
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
        ["matrix" | "languages"] | ["language" | "fact", _] => Err(
            CupelError::pending_description(&format!("describe {}", words[0]), "P1.1"),
        ),
        ["finding", _] => Err(CupelError::pending_description("describe finding", "P2.1")),
        [check] if check.contains('.') => {
            Err(CupelError::pending_description("describe CHECK", "P2.1"))
        }
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
