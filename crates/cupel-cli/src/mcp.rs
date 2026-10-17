// ABOUTME: The verbs `serve --mcp` exposes as MCP tools (ADR-121): each tool's input schema derived from the
// ABOUTME: registry's description of its verb, and a call's arguments turned into the command line it runs.
use cupel_core::{ArgDescription, ExitCode, ValueType};
use cupel_serve::mcp::{Arguments, Tool};
use serde_json::{Map, Value, json};

use crate::error::{CupelError, Usage};
use crate::registry;

/// A tool: its name, the command words it runs, and the switches those words imply.
struct Spec {
    name: &'static str,
    words: &'static [&'static str],
    implied: &'static [&'static str],
}

/// The tools, in the order `tools/list` lists them (design §8.10).
const SPECS: [Spec; 5] = [
    Spec {
        name: "check",
        words: &["check"],
        implied: &[],
    },
    Spec {
        name: "survey",
        words: &["survey"],
        implied: &[],
    },
    Spec {
        name: "explain",
        words: &["explain"],
        implied: &[],
    },
    Spec {
        name: "tests_impacted",
        words: &["tests"],
        implied: &["--impacted"],
    },
    Spec {
        name: "describe",
        words: &["describe"],
        implied: &[],
    },
];

/// Flags no tool takes. The result is always the JSON envelope (`--format`, `--no-color`), events
/// arrive as progress notifications (`--events`), the `describe` tool describes (`--describe`,
/// `--help`), and a process sizes its thread pool once, so the server's first call would fix it
/// for every later one (`--jobs`, ADR-086 §11).
const WITHHELD: [&str; 6] = [
    "--format",
    "--no-color",
    "--events",
    "--describe",
    "--help",
    "--jobs",
];

/// Every tool with its input schema.
pub fn tools() -> Vec<Tool> {
    SPECS.iter().map(tool).collect()
}

fn tool(spec: &Spec) -> Tool {
    let command = described(spec);
    let words = [spec.words, spec.implied].concat().join(" ");
    let arguments = arguments(spec);
    let properties: Map<String, Value> = arguments
        .iter()
        .map(|arg| (property(arg), schema(arg)))
        .collect();
    let required: Vec<String> = arguments
        .iter()
        .filter(|arg| arg.required)
        .map(property)
        .collect();
    let mut input_schema = json!({
        "type": "object",
        "properties": properties,
        "additionalProperties": false,
    });
    if !required.is_empty() {
        input_schema["required"] = json!(required);
    }
    Tool {
        name: spec.name.to_owned(),
        title: format!("cupel {words}"),
        description: format!(
            "{} Returns the envelope `cupel {words} --format json` prints; its exit_code and \
             status say how the run ended.",
            command.summary
        ),
        input_schema,
    }
}

/// The registry's description of `spec`'s command.
fn described(spec: &Spec) -> cupel_core::CommandDescription {
    let words: Vec<String> = spec.words.iter().map(|word| (*word).to_owned()).collect();
    registry::find(&words).expect("every tool's command is registered")
}

/// The arguments `spec`'s tool takes: its command's and the global flags, less the withheld,
/// the implied, and those not yet implemented.
fn arguments(spec: &Spec) -> Vec<ArgDescription> {
    described(spec)
        .arguments
        .into_iter()
        .chain(registry::cli().global_flags)
        .filter(|arg| {
            !WITHHELD.contains(&arg.name.as_str())
                && !spec.implied.contains(&arg.name.as_str())
                && arg.pending_step.is_none()
        })
        .collect()
}

/// The property naming `arg`: `--no-gate` is `no_gate`, the positional `TARGET` is `target`.
fn property(arg: &ArgDescription) -> String {
    arg.name
        .trim_start_matches("--")
        .replace('-', "_")
        .to_lowercase()
}

fn schema(arg: &ArgDescription) -> Value {
    let mut value = match arg.value {
        ValueType::Switch => json!({"type": "boolean"}),
        ValueType::Integer => json!({"type": "integer", "minimum": 0}),
        ValueType::Enum => json!({"type": "string", "enum": arg.values}),
        ValueType::String | ValueType::Path | ValueType::Duration => json!({"type": "string"}),
    };
    if arg.repeated {
        value = json!({"type": "array", "items": value});
    }
    value["description"] = json!(arg.summary);
    value
}

/// Whether a run that exited with `code` is a tool execution error: a usage, environment,
/// unevaluable, or internal error (design §6.6). A failed gate or a partial run is a result.
pub fn is_error(code: u8) -> bool {
    [
        ExitCode::Usage,
        ExitCode::Environment,
        ExitCode::Unevaluable,
        ExitCode::Internal,
    ]
    .iter()
    .any(|error| error.code() == code)
}

/// The command line, after the program name, the tool `name` runs for `arguments`: its words,
/// its positional arguments, its flags in the order given, then `--format json`. Before the
/// argument at fault, the command line it ran up to there and why it stopped.
///
/// # Panics
///
/// When `name` is not one of [`tools`].
pub fn command(
    name: &str,
    arguments: &Arguments,
) -> Result<Vec<String>, (Vec<String>, CupelError)> {
    let spec = SPECS
        .iter()
        .find(|spec| spec.name == name)
        .expect("a known tool");
    let accepted = self::arguments(spec);
    let mut positionals: Vec<String> = Vec::new();
    let mut flags: Vec<String> = spec.implied.iter().map(|flag| (*flag).to_owned()).collect();
    let mut failure = None;
    for (key, value) in &arguments.0 {
        let Some(arg) = accepted.iter().find(|arg| property(arg) == *key) else {
            failure = Some(unknown(name, key, value, &accepted));
            break;
        };
        match words(arg, value) {
            Ok(words) if arg.positional => positionals.extend(words),
            Ok(words) => flags.extend(words),
            Err(usage) => {
                failure = Some(usage);
                break;
            }
        }
    }
    let line: Vec<String> = spec
        .words
        .iter()
        .map(|word| (*word).to_owned())
        .chain(positionals)
        .chain(flags)
        .chain(["--format".to_owned(), "json".to_owned()])
        .collect();
    match failure {
        None => Ok(line),
        Some(usage) => Err((line, usage)),
    }
}

/// The words `value` of `arg` adds to the command line: the flag for a true switch, none for a
/// false one or a null, else the flag (none for a positional) and each value.
fn words(arg: &ArgDescription, value: &Value) -> Result<Vec<String>, CupelError> {
    if arg.value == ValueType::Switch {
        return match value {
            Value::Bool(true) => Ok(vec![arg.name.clone()]),
            Value::Bool(false) | Value::Null => Ok(Vec::new()),
            _ => Err(mistyped(arg, value)),
        };
    }
    let items: Vec<&Value> = match value {
        Value::Null => Vec::new(),
        Value::Array(items) if arg.repeated => items.iter().collect(),
        _ if arg.repeated => return Err(mistyped(arg, value)),
        single => vec![single],
    };
    let values = items
        .into_iter()
        .map(|item| scalar(arg, item))
        .collect::<Option<Vec<String>>>()
        .ok_or_else(|| mistyped(arg, value))?;
    if values.is_empty() {
        return Ok(Vec::new());
    }
    let flag = (!arg.positional).then(|| arg.name.clone());
    Ok(flag.into_iter().chain(values).collect())
}

/// `value` as one command-line word, if it has `arg`'s type: a non-negative integer for an
/// integer, a string otherwise.
fn scalar(arg: &ArgDescription, value: &Value) -> Option<String> {
    match (arg.value, value) {
        (ValueType::Integer, Value::Number(number)) => number.as_u64().map(|n| n.to_string()),
        (ValueType::Integer, _) => None,
        (_, Value::String(text)) => Some(text.clone()),
        _ => None,
    }
}

fn mistyped(arg: &ArgDescription, value: &Value) -> CupelError {
    let property = property(arg);
    let (one, many) = match arg.value {
        ValueType::Switch => ("a boolean", "booleans"),
        ValueType::Integer => ("a non-negative integer", "non-negative integers"),
        _ => ("a string", "strings"),
    };
    let wanted = if arg.repeated {
        format!("an array of {many}")
    } else {
        one.to_owned()
    };
    Usage {
        subject: property.clone(),
        title: format!("`{property}` must be {wanted}"),
        fix: format!("Pass `{property}` as {wanted}, as the tool's input schema says."),
        detail: None,
        argument: Some(property),
        value: Some(value.to_string()),
        valid: Vec::new(),
    }
    .into()
}

fn unknown(tool: &str, key: &str, value: &Value, accepted: &[ArgDescription]) -> CupelError {
    Usage {
        subject: key.to_owned(),
        title: format!("The `{tool}` tool takes no argument `{key}`"),
        fix: "Pass only the properties of the tool's input schema.".to_owned(),
        detail: None,
        argument: Some(key.to_owned()),
        value: Some(value.to_string()),
        valid: accepted.iter().map(property).collect(),
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_named(name: &str) -> Tool {
        tools().into_iter().find(|tool| tool.name == name).unwrap()
    }

    fn properties(name: &str) -> Vec<String> {
        let schema = tool_named(name).input_schema;
        schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }

    fn arguments(pairs: &[(&str, Value)]) -> Arguments {
        Arguments(
            pairs
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect(),
        )
    }

    fn line(name: &str, pairs: &[(&str, Value)]) -> Vec<String> {
        command(name, &arguments(pairs)).unwrap()
    }

    #[test]
    fn the_tools_are_the_verbs_design_names() {
        let names: Vec<String> = tools().into_iter().map(|tool| tool.name).collect();
        assert_eq!(
            names,
            ["check", "survey", "explain", "tests_impacted", "describe"]
        );
        assert_eq!(tool_named("tests_impacted").title, "cupel tests --impacted");
        let description = tool_named("check").description;
        assert!(
            description.starts_with("Run checks over a scope at a depth and evaluate the gate. "),
            "{description}"
        );
    }

    #[test]
    fn a_schema_takes_the_verbs_flags_and_the_global_ones_but_not_the_withheld_or_pending() {
        let check = properties("check");
        for name in [
            "changed", "only", "depth", "no_gate", "scaffold", "cwd", "dry_run",
        ] {
            assert!(check.contains(&name.to_owned()), "{name} in {check:?}");
        }
        for name in [
            "format",
            "events",
            "describe",
            "help",
            "no_color",
            "jobs",
            "since_baseline",
        ] {
            assert!(!check.contains(&name.to_owned()), "{name} in {check:?}");
        }
        assert!(!properties("tests_impacted").contains(&"impacted".to_owned()));
        assert!(!properties("describe").contains(&"agent".to_owned()));
    }

    #[test]
    fn property_schemas_follow_the_value_types() {
        let schema = tool_named("check").input_schema;
        let properties = &schema["properties"];
        assert_eq!(properties["changed"]["type"], "boolean");
        assert_eq!(properties["only"]["type"], "array");
        assert_eq!(properties["only"]["items"], json!({"type": "string"}));
        assert_eq!(
            properties["depth"]["enum"],
            json!(["quick", "standard", "deep", "exhaustive"])
        );
        assert_eq!(properties["limit"]["type"], "integer");
        assert_eq!(properties["timeout"]["type"], "string");
        assert_eq!(schema["additionalProperties"], false);
        assert!(schema.get("required").is_none());
        let explain = tool_named("explain").input_schema;
        assert_eq!(explain["required"], json!(["target"]));
        assert_eq!(
            tool_named("describe").input_schema["properties"]["subject"]["type"],
            "array"
        );
    }

    #[test]
    fn arguments_become_the_command_line_in_the_order_given() {
        assert_eq!(
            line(
                "check",
                &[
                    ("only", json!(["b", "a"])),
                    ("changed", json!(true)),
                    ("gate", json!(false)),
                    ("depth", json!("quick")),
                    ("limit", json!(5)),
                    ("base", Value::Null),
                ]
            ),
            [
                "check",
                "--only",
                "b",
                "a",
                "--changed",
                "--depth",
                "quick",
                "--limit",
                "5",
                "--format",
                "json"
            ]
        );
        assert_eq!(line("survey", &[]), ["survey", "--format", "json"]);
        assert_eq!(
            line(
                "explain",
                &[("only", json!(["x"])), ("target", json!("fnd_1"))]
            ),
            ["explain", "fnd_1", "--only", "x", "--format", "json"]
        );
        assert_eq!(
            line("tests_impacted", &[("changed", json!(true))]),
            ["tests", "--impacted", "--changed", "--format", "json"]
        );
        assert_eq!(
            line("describe", &[("subject", json!(["language", "rust"]))]),
            ["describe", "language", "rust", "--format", "json"]
        );
        assert_eq!(
            line("check", &[("only", json!([]))]),
            ["check", "--format", "json"]
        );
    }

    #[test]
    fn an_argument_outside_the_schema_or_of_another_type_is_a_usage_error() {
        let failed = |pairs: &[(&str, Value)]| {
            let (line, error) = command("check", &arguments(pairs)).unwrap_err();
            let CupelError::Usage(usage) = error else {
                panic!("a usage error")
            };
            (line, usage)
        };
        let (line, usage) = failed(&[("changed", json!(true)), ("format", json!("text"))]);
        assert_eq!(line, ["check", "--changed", "--format", "json"]);
        assert_eq!(usage.title, "The `check` tool takes no argument `format`");
        assert_eq!(usage.argument.as_deref(), Some("format"));
        assert!(usage.valid.contains(&"only".to_owned()));
        let cases = [
            ("changed", json!("yes"), "`changed` must be a boolean"),
            ("only", json!("a"), "`only` must be an array of strings"),
            ("only", json!([1]), "`only` must be an array of strings"),
            ("limit", json!(-1), "`limit` must be a non-negative integer"),
            ("depth", json!(3), "`depth` must be a string"),
        ];
        for (key, value, title) in cases {
            let (_, usage) = failed(&[(key, value.clone())]);
            assert_eq!(usage.title, title, "{key}: {value}");
            assert_eq!(usage.value, Some(value.to_string()));
        }
    }

    #[test]
    fn errors_are_the_exit_codes_that_say_the_run_could_not_grade() {
        let errors: Vec<u8> = (0..=6).filter(|code| is_error(*code)).collect();
        assert_eq!(errors, [2, 3, 4, 6]);
    }
}
