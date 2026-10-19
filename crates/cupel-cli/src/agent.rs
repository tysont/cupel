// ABOUTME: `cupel describe --agent`: the Markdown guide an agent harness pastes into CLAUDE.md or AGENTS.md.
// ABOUTME: The prose lives here; verbs, flags, exit codes, statuses, checks, and MCP tools are rendered from the registries.
use cupel_core::{ArgDescription, CheckRegistry, CliDescription, CostClass, schema};
use serde_json::Value;

use crate::{mcp, registry};

/// The guide, about sixty lines of Markdown ending in one newline.
pub fn guide(checks: &CheckRegistry) -> String {
    let cli = registry::cli();
    format!("{}{}", the_loop(&cli), the_results(&cli, checks))
}

/// What Cupel is, the loop an agent runs, and what each exit code asks of it.
fn the_loop(cli: &CliDescription) -> String {
    format!(
        "# Cupel

{summary} It measures structure and test strength over a change or a whole tree and returns one JSON envelope: findings with stable IDs, ranked `next_actions`, and a gate verdict. It never edits code; you fix, it grades.

## The loop

1. Run `cupel check --changed --format json`.
2. If `exit_code` is 0, stop: the gate passed, or there is no gate.
3. Otherwise take `next_actions[0]`, fix what it names, and rerun its finding's `repro` to confirm.
4. Go back to 1. Do not stop until it exits 0.

- `cupel explain <finding-id>` shows a finding's source, covering tests, and a test skeleton.
- `cupel tests --impacted --changed` lists the tests the change affects and the command that runs them.
- `cupel describe <check-id>` and `cupel describe finding <kind>` explain any check or finding kind.
- `--dry-run` shows what any command would run or write; `<verb> --describe` lists its flags.

## Exit codes

{exit_codes}
Exit 2 is a mistake in your command line or `cupel.toml`; its finding names the fix. On exit 3 run `cupel doctor` and follow its `install_tool` and `configure` actions. On exit 4 make the test suite pass first.

",
        summary = cli.summary,
        exit_codes = exit_codes(cli),
    )
}

/// The gate, the depths, how to read the envelope, the MCP server, and the verbs.
fn the_results(cli: &CliDescription, checks: &CheckRegistry) -> String {
    format!(
        "## The gate

`[gate.changed]` in `cupel.toml` applies to `--changed` and `--base` scopes, `[gate.tree]` to the rest. A rule maps a check, or `<check-id>.<metric>`, to `none`, `no-breaking`, or a comparison such as `\"static.complexity.cyclomatic_max\" = \"<= 10\"` or `\"tests.weakening\" = \"none\"`. `--gate` is the default when the table exists and `--no-gate` reports without a verdict. A rule on a check that did not run is skipped, and every rule that does not pass fails the gate (exit 1); `summary.gate` has each rule's result. Do not loosen the gate or suppress findings to pass it unless asked.

## Depth

{depth}
- Scope flags (`--changed`, `--base`, `--paths`, `--symbols`) intersect; `--only` and `--skip` select checks by ID or group.

{depth_table}
## The envelope

Stdout is one JSON envelope; diagnostics go to stderr. Read:

- `status`: {statuses}; `exit_code` always equals the process's.
- `findings[]`: `id`, `check`, `kind`, `severity`, `location`, `title`, `action`, and `repro`, the command that reruns only that check on only that symbol.
- `next_actions[]`: what to fix, most important first; each has a `kind`, a `summary`, and its `finding`.
- `summary.gate`: the verdict and each rule's result; `truncated` says `--limit` cut the findings.

## MCP

A client that calls tools can run `cupel serve --mcp` (JSON-RPC on stdio) instead of the commands. Its tools are {tools}; each takes its verb's flags as arguments and returns the envelope the command prints.

## More

The verbs are {verbs}. `cupel describe --all` is every verb, flag, exit code, and schema as one JSON document.
",
        depth = depth(),
        depth_table = depth_table(checks),
        statuses = code_list(&statuses()),
        tools = code_list(&mcp::tools().into_iter().map(|tool| tool.name).collect::<Vec<_>>()),
        verbs = verbs(cli),
    )
}

/// Items as inline code, joined with commas.
fn code_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("`{item}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A Markdown table: the header row, the separator, then one line per row.
fn table(header: &[&str], rows: impl IntoIterator<Item = Vec<String>>) -> String {
    let line = |cells: &[String]| format!("| {} |\n", cells.join(" | "));
    let header: Vec<String> = header.iter().map(|cell| (*cell).to_owned()).collect();
    let separator = vec!["---".to_owned(); header.len()];
    let body: String = rows.into_iter().map(|row| line(&row)).collect();
    format!("{}{}{body}", line(&header), line(&separator))
}

fn exit_codes(cli: &CliDescription) -> String {
    table(
        &["Code", "Name", "Meaning"],
        cli.exit_codes.iter().map(|code| {
            vec![
                code.code.code().to_string(),
                code.name.clone(),
                code.meaning.clone(),
            ]
        }),
    )
}

/// `--depth` and `--budget` as `check` registers them, one list item each.
fn depth() -> String {
    let check = registry::find(&["check".to_owned()]).expect("check is registered");
    let argument = |name: &str| -> &ArgDescription {
        check
            .arguments
            .iter()
            .find(|argument| argument.name == name)
            .expect("check takes its scope flags")
    };
    let depth = argument("--depth");
    let budget = argument("--budget");
    format!(
        "- `{} {}`, default `{}`. {}\n- `{} {}`. {}",
        depth.name,
        depth.values.join("|"),
        depth.default.as_deref().unwrap_or_default(),
        depth.summary,
        budget.name,
        budget.value_name.as_deref().unwrap_or_default(),
        budget.summary,
    )
}

/// The checks each depth adds, from the registered checks' cost classes.
fn depth_table(checks: &CheckRegistry) -> String {
    let classes = [
        CostClass::Quick,
        CostClass::Standard,
        CostClass::Deep,
        CostClass::Exhaustive,
    ];
    table(
        &["Depth", "Adds"],
        classes.into_iter().map(|class| {
            let ids: Vec<String> = checks
                .iter()
                .map(cupel_core::Check::manifest)
                .filter(|manifest| manifest.cost == class)
                .map(|manifest| manifest.id.to_string())
                .collect();
            vec![format!("`{class}`"), code_list(&ids)]
        }),
    )
}

/// The envelope's `status` values, from the run schema.
fn statuses() -> Vec<String> {
    let run = schema::run().to_value();
    let mut names = Vec::new();
    for variant in run["$defs"]["Status"]["oneOf"]
        .as_array()
        .expect("Status is a oneOf")
    {
        match (&variant["enum"], &variant["const"]) {
            (Value::Array(values), _) => {
                names.extend(values.iter().filter_map(Value::as_str).map(str::to_owned));
            }
            (_, Value::String(value)) => names.push(value.clone()),
            _ => {}
        }
    }
    names
}

fn verbs(cli: &CliDescription) -> String {
    code_list(
        &cli.verbs
            .iter()
            .map(|verb| verb.name.clone())
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_are_every_status_the_envelope_can_carry() {
        let expected: Vec<String> = [
            cupel_core::Status::Pass,
            cupel_core::Status::Fail,
            cupel_core::Status::Partial,
            cupel_core::Status::Error,
            cupel_core::Status::Plan,
        ]
        .iter()
        .map(|status| {
            serde_json::to_value(status)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
        assert_eq!(statuses(), expected);
    }
}
