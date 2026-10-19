// ABOUTME: Integration tests for `cupel describe --agent`: the Markdown guide for CLAUDE.md and AGENTS.md,
// ABOUTME: its length, its JSON and text forms, and that every flag, check, and tool it names is registered.
mod support;

use std::collections::BTreeSet;

use serde_json::{Value, json};
use support::{check_ids, described, run};

/// The guide's target length (design §6.5: "~60 lines") and how far it may stray.
const TARGET_LINES: usize = 60;
const TOLERANCE: usize = 10;

/// The guide as `--format text` prints it.
fn guide() -> String {
    let outcome = run(["describe", "--agent", "--format", "text"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(outcome.stderr, "");
    outcome.stdout
}

/// The spans of `text` written as inline code.
fn code_spans(text: &str) -> Vec<&str> {
    text.split('`').skip(1).step_by(2).collect()
}

/// The words of `span`, split on whatever cannot be part of a flag, check ID, or metric.
fn words(span: &str) -> impl Iterator<Item = &str> {
    span.split(|c: char| !(c.is_ascii_alphanumeric() || "-._".contains(c)))
        .map(|word| word.trim_end_matches('.'))
        .filter(|word| !word.is_empty())
}

/// Every flag the registry describes: the global flags, the CLI's own, and every command's.
fn registered_flags() -> BTreeSet<String> {
    let all = described(&["--all"]);
    let mut flags = BTreeSet::new();
    let mut add = |arguments: &Value| {
        for argument in arguments.as_array().expect("arguments") {
            flags.insert(argument["name"].as_str().expect("name").to_owned());
        }
    };
    add(&all["cli"]["global_flags"]);
    add(&all["cli"]["flags"]);
    let mut commands: Vec<Value> = all["commands"].as_array().expect("commands").clone();
    while let Some(command) = commands.pop() {
        add(&command["arguments"]);
        commands.extend(
            command["subcommands"]
                .as_array()
                .expect("subcommands")
                .clone(),
        );
    }
    flags
}

#[test]
fn the_guide_is_about_sixty_lines_of_markdown() {
    let guide = guide();
    let lines = guide.lines().count();
    assert!(
        lines.abs_diff(TARGET_LINES) <= TOLERANCE,
        "{lines} lines, not {TARGET_LINES} ± {TOLERANCE}:\n{guide}"
    );
    assert!(guide.starts_with("# Cupel\n"), "{guide}");
    assert!(
        guide.ends_with('\n') && !guide.ends_with("\n\n"),
        "{guide:?}"
    );
    insta::assert_snapshot!(guide);
}

#[test]
fn json_carries_the_same_markdown_as_the_agent_description() {
    let outcome = run(["describe", "--agent", "--format", "json"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("pass"));
    assert_eq!(
        envelope["description"],
        json!({"subject": "agent", "markdown": guide()})
    );
}

#[test]
fn every_flag_the_guide_names_is_registered() {
    let registered = registered_flags();
    let guide = guide();
    let named: BTreeSet<&str> = code_spans(&guide)
        .into_iter()
        .flat_map(words)
        .filter(|word| word.starts_with("--"))
        .collect();
    let unregistered: Vec<&&str> = named
        .iter()
        .filter(|flag| !registered.contains(**flag))
        .collect();
    assert_eq!(
        unregistered,
        Vec::<&&str>::new(),
        "registered: {registered:?}"
    );
    assert!(
        named.contains("--changed") && named.contains("--depth"),
        "{named:?}"
    );
}

#[test]
fn every_check_and_gate_metric_the_guide_names_is_registered() {
    let checks = check_ids();
    let groups: BTreeSet<&str> = checks
        .iter()
        .map(|id| id.split('.').next().expect("a group"))
        .collect();
    let guide = guide();
    let named: BTreeSet<&str> = code_spans(&guide)
        .into_iter()
        .flat_map(words)
        .filter(|word| {
            word.split_once('.')
                .is_some_and(|(group, _)| groups.contains(group))
        })
        .collect();
    let unregistered: Vec<&&str> = named
        .iter()
        .filter(|name| {
            if checks.iter().any(|id| id == **name) {
                return false;
            }
            let Some((check, metric)) = name.rsplit_once('.') else {
                return true;
            };
            !checks.iter().any(|id| id == check)
                || !described(&[check])["manifest"]["metrics"]
                    .as_array()
                    .expect("metrics")
                    .iter()
                    .any(|registered| registered["name"] == metric)
        })
        .collect();
    assert_eq!(unregistered, Vec::<&&str>::new());
    assert!(!named.is_empty(), "the guide's gate example names checks");
}

#[test]
fn the_guide_lists_every_exit_code_and_mcp_tool_by_name() {
    let guide = guide();
    let cli = described(&[]);
    let codes: Vec<String> = cli["exit_codes"]
        .as_array()
        .expect("exit codes")
        .iter()
        .map(|code| {
            format!(
                "| {} | {} | {} |",
                code["code"],
                code["name"].as_str().unwrap(),
                code["meaning"].as_str().unwrap()
            )
        })
        .filter(|row| !guide.contains(row.as_str()))
        .collect();
    assert_eq!(codes, Vec::<String>::new(), "rows missing from:\n{guide}");
    let tools: Vec<String> = support::listed_tools()
        .iter()
        .map(|tool| tool["name"].as_str().expect("a name").to_owned())
        .collect();
    let missing: Vec<&String> = tools
        .iter()
        .filter(|tool| !guide.contains(&format!("`{tool}`")))
        .collect();
    assert_eq!(tools.len(), 5);
    assert_eq!(missing, Vec::<&String>::new());
}
