// ABOUTME: `oracle add` and `oracle remove` (ADR-090 §2): one `[oracles.<name>]` table of cupel.toml changed in
// ABOUTME: place, everything else kept byte for byte; previewed as a diff without --write.
use std::path::PathBuf;

use cupel_core::config::{Config, OracleConfig};
use cupel_core::{
    ActionKind, Envelope, EventSink, ExitCode, NextAction, OracleReport, RunPlan, Status, Writer,
};
use toml_edit::{DocumentMut, Item, Table, value};

use super::{defined, report};
use crate::error::CupelError;
use crate::persist;
use crate::run::{Invocation, shell_quote};

/// The section oracles are defined under.
const ORACLES: &str = "oracles";

/// Adds `definition` as the oracle `name`, or previews that.
pub(super) fn add(
    invocation: &Invocation<'_>,
    config: &Config,
    name: &str,
    definition: &OracleConfig,
    events: &mut dyn EventSink,
) -> Result<Envelope, CupelError> {
    if config.file.oracles.contains_key(name) {
        return Err(CupelError::bad_argument(
            "NAME",
            Some(name),
            format!("An oracle named `{name}` is already defined"),
            format!("Choose another name, or run `cupel oracle remove {name} --write` first."),
        ));
    }
    OracleConfig::validate_name(name).map_err(|reason| {
        CupelError::bad_argument(
            "NAME",
            Some(name),
            reason,
            "Name the oracle with lowercase letters, digits, `_`, and `-`, starting with a letter."
                .to_owned(),
        )
    })?;
    definition.validate().map_err(|reason| {
        CupelError::bad_argument(
            "--kind",
            Some(definition.kind.as_str()),
            format!("Oracle `{name}`: {reason}"),
            "Give each kind its own flags: `ref` takes --ref; `command` takes --cmd and \
             --input; `spec` takes --spec and --lang (tla+, quint, or p)."
                .to_owned(),
        )
    })?;
    let text = config_text(invocation, config);
    let mut document = parsed(&text)?;
    let oracles = document
        .entry(ORACLES)
        .or_insert_with(|| {
            let mut table = Table::new();
            table.set_implicit(true);
            Item::Table(table)
        })
        .as_table_mut()
        .ok_or_else(|| not_a_table(config))?;
    let mut table = table_of(definition);
    if !text.trim().is_empty() {
        table.decor_mut().set_prefix("\n");
    }
    oracles.insert(name, Item::Table(table));
    let store = persist::open_for_reading(invocation.writer)?;
    let added = report(invocation.writer.root(), store.as_ref(), name, definition)?;
    let change = Change {
        path: config_path(invocation, config),
        text: document.to_string(),
        oracle: added,
        command: format!("{} --write", add_command(name, definition)),
    };
    finish(invocation, change, events)
}

/// Removes the oracle `name` and its verification, or previews that.
pub(super) fn remove(
    invocation: &Invocation<'_>,
    config: &Config,
    name: &str,
    events: &mut dyn EventSink,
) -> Result<Envelope, CupelError> {
    let (_, definition) = defined(config, name, "NAME")?;
    let store = persist::open_for_reading(invocation.writer)?;
    let removed = report(invocation.writer.root(), store.as_ref(), name, definition)?;
    let mut document = parsed(&config_text(invocation, config))?;
    let oracles = document
        .get_mut(ORACLES)
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| not_a_table(config))?;
    oracles.remove(name);
    if oracles.is_empty() {
        document.remove(ORACLES);
    }
    let change = Change {
        path: config_path(invocation, config),
        text: document.to_string(),
        oracle: removed,
        command: format!("cupel oracle remove {} --write", shell_quote(name)),
    };
    let envelope = finish(invocation, change, events)?;
    if let Some(store) = store.filter(|_| envelope.status != Status::Plan) {
        store
            .forget_oracle(name)
            .map_err(|error| persist::unusable(&error))?;
    }
    Ok(envelope)
}

/// One edit of the configuration file: its new text, the oracle it adds or removes, and the
/// command that makes it.
struct Change {
    path: PathBuf,
    text: String,
    oracle: OracleReport,
    command: String,
}

/// Writes `change`, or previews it with its command as the action that applies it; the
/// envelope reports its oracle.
fn finish(
    invocation: &Invocation<'_>,
    change: Change,
    events: &mut dyn EventSink,
) -> Result<Envelope, CupelError> {
    let (run, writer) = (invocation.run, invocation.writer);
    let Change {
        path,
        text,
        oracle,
        command,
    } = change;
    let mut envelope = if writer.is_recording() {
        // A preview shows what `--write` would do, so its recording writer may plan writes
        // outside `.cupel/`; it never writes anything.
        let preview = Writer::new(writer.root(), true)?.recording();
        preview.write(&path, text.as_bytes())?;
        let mut envelope = run.plan_envelope(RunPlan::default(), &preview, events, Vec::new());
        envelope.next_actions.push(NextAction {
            priority: 1,
            finding: None,
            kind: ActionKind::Configure,
            summary: format!("Nothing was written; run `{command}` to change cupel.toml."),
            command: Some(command),
        });
        envelope
    } else {
        writer.write(&path, text.as_bytes())?;
        run.reported(Status::Pass, ExitCode::Pass, Vec::new())
    };
    envelope.oracles = Some(vec![oracle]);
    Ok(envelope)
}

/// The `oracle add` command line that adds `definition` as `name`.
fn add_command(name: &str, definition: &OracleConfig) -> String {
    let mut words = vec![
        "cupel oracle add".to_owned(),
        shell_quote(name),
        format!("--kind {}", definition.kind.as_str()),
    ];
    let flags = [
        ("--ref", &definition.reference),
        ("--cmd", &definition.cmd),
        ("--spec", &definition.spec),
        ("--lang", &definition.lang),
    ];
    for (flag, value) in flags {
        if let Some(value) = value {
            words.push(format!("{flag} {}", shell_quote(value)));
        }
    }
    for input in &definition.inputs {
        words.push(format!("--input {}", shell_quote(input)));
    }
    words.join(" ")
}

/// The `[oracles.<name>]` table for `definition`, keys in the order `cupel.toml` documents them.
fn table_of(definition: &OracleConfig) -> Table {
    let mut table = Table::new();
    table.insert("kind", value(definition.kind.as_str()));
    let keys = [
        ("ref", &definition.reference),
        ("cmd", &definition.cmd),
        ("spec", &definition.spec),
        ("lang", &definition.lang),
    ];
    for (key, given) in keys {
        if let Some(given) = given {
            table.insert(key, value(given.as_str()));
        }
    }
    if !definition.inputs.is_empty() {
        let inputs: toml_edit::Array = definition.inputs.iter().map(String::as_str).collect();
        table.insert("inputs", value(inputs));
    }
    table
}

/// The configuration file `oracle add` and `oracle remove` change: the one that loaded, else
/// `cupel.toml` at the root.
fn config_path(invocation: &Invocation<'_>, config: &Config) -> PathBuf {
    config
        .path
        .clone()
        .unwrap_or_else(|| invocation.writer.root().join("cupel.toml"))
}

/// The configuration file's text; empty when there is none.
fn config_text(invocation: &Invocation<'_>, config: &Config) -> String {
    std::fs::read_to_string(config_path(invocation, config)).unwrap_or_default()
}

/// `text` as an editable document.
fn parsed(text: &str) -> Result<DocumentMut, CupelError> {
    text.parse::<DocumentMut>().map_err(|error| {
        CupelError::bad_argument(
            "--config",
            None,
            format!("The configuration cannot be edited: {error}"),
            "Fix cupel.toml so it parses.".to_owned(),
        )
    })
}

/// The usage error for an `oracles` key that is not a table.
fn not_a_table(config: &Config) -> CupelError {
    let shown = config.path.as_deref().map_or_else(
        || "cupel.toml".to_owned(),
        |path| path.display().to_string(),
    );
    CupelError::bad_argument(
        "--config",
        None,
        format!("`oracles` in {shown} is not a table"),
        "Define oracles as `[oracles.<name>]` tables.".to_owned(),
    )
}
