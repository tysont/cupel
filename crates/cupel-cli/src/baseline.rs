// ABOUTME: The `baseline` verb (design §6.5, ADR-046): set (previewed without --write), show, diff two
// ABOUTME: runs as new and resolved findings, and export or import a baseline run as JSON.
use std::path::Path;

use cupel_core::{
    ActionKind, BaselineRef, Envelope, EventSink, ExitCode, NextAction, PlannedWrite, RunId,
    RunPlan, Status, WriteError, Writer, deltas,
};
use cupel_store::Store;

use crate::against::{self, DEFAULT_BASELINE};
use crate::cli::BaselineArgs;
use crate::error::CupelError;
use crate::persist;
use crate::run::Run;

/// Runs `action` on the baseline `name` (default `default`). A recording writer (a dry run, or
/// `set`/`import` without `--write`) plans instead of changing anything, reading the store only
/// if it already exists.
pub fn run(
    run: &Run,
    writer: &Writer,
    events: &mut dyn EventSink,
    name: Option<&str>,
    action: &BaselineArgs,
) -> Result<Envelope, CupelError> {
    let name = name.unwrap_or(DEFAULT_BASELINE);
    let preview = writer.is_recording();
    let stored = |error: cupel_store::StoreError| persist::unusable(&error);
    let named = if name == DEFAULT_BASELINE {
        "cupel baseline".to_owned()
    } else {
        format!("cupel baseline --name {name}")
    };
    match action {
        BaselineArgs::Set { from } if preview => {
            let store = persist::open_for_reading(writer)?;
            let target = chosen(store.as_ref(), from.as_deref())?;
            let from = from
                .as_ref()
                .map(|from| format!(" --from {from}"))
                .unwrap_or_default();
            let command = format!("{named} set{from} --write");
            let current = current(store.as_ref(), name)?;
            Ok(previewed(
                run, writer, events, name, current, target, &command,
            ))
        }
        BaselineArgs::Set { from } => {
            let store = persist::open(writer)?;
            let target = chosen(Some(&store), from.as_deref())?;
            store.set_baseline(name, &target).map_err(stored)?;
            show(run, &store, name)
        }
        BaselineArgs::Show | BaselineArgs::Diff { .. } if preview => {
            Ok(run.plan_envelope(RunPlan::default(), writer, events, Vec::new()))
        }
        BaselineArgs::Show => show(run, &persist::open(writer)?, name),
        BaselineArgs::Diff { against } => diff(
            run,
            &persist::open(writer)?,
            writer.root(),
            name,
            against.as_deref(),
        ),
        BaselineArgs::Export { file } => {
            let store = persist::open_for_reading(writer)?.ok_or_else(|| unset(name))?;
            let (id, envelope) = baseline_run(&store, name)?;
            let mut json = serde_json::to_string_pretty(&envelope)
                .map_err(|error| bad_file(file, &error.to_string()))?;
            json.push('\n');
            writer
                .write(file, json.as_bytes())
                .map_err(|error| refused(file, &error))?;
            if preview {
                return Ok(run.plan_envelope(RunPlan::default(), writer, events, Vec::new()));
            }
            let mut exported = run.reported(Status::Pass, ExitCode::Pass, Vec::new());
            exported.run.baseline = Some(BaselineRef {
                id,
                name: name.to_owned(),
            });
            Ok(exported)
        }
        BaselineArgs::Import { file } => {
            let text = std::fs::read_to_string(writer.root().join(file))
                .map_err(|error| bad_file(file, &error.to_string()))?;
            let imported: Envelope =
                serde_json::from_str(&text).map_err(|error| bad_file(file, &error.to_string()))?;
            if preview {
                let store = persist::open_for_reading(writer)?;
                let command = format!("{named} import {} --write", file.display());
                let current = current(store.as_ref(), name)?;
                return Ok(previewed(
                    run,
                    writer,
                    events,
                    name,
                    current,
                    imported.run.id,
                    &command,
                ));
            }
            let store = persist::open(writer)?;
            store.record_run(&imported).map_err(stored)?;
            store.set_baseline(name, &imported.run.id).map_err(stored)?;
            show(run, &store, name)
        }
    }
}

/// The run the baseline `name` points at now, if the store exists and has it.
fn current(store: Option<&Store>, name: &str) -> Result<Option<RunId>, CupelError> {
    Ok(store
        .map(|store| store.baseline(name))
        .transpose()
        .map_err(|error| persist::unusable(&error))?
        .flatten())
}

/// The run `set` would name: `--from run:ID` when given (it must be stored), else the latest
/// result run.
fn chosen(store: Option<&Store>, from: Option<&str>) -> Result<RunId, CupelError> {
    let stored = |error: cupel_store::StoreError| persist::unusable(&error);
    let refuse = |title: String| {
        CupelError::bad_argument(
            "--from",
            from,
            title,
            "Run `cupel doctor` or `cupel check` first, or pass --from run:<run ID> naming a \
             stored run."
                .to_owned(),
        )
    };
    match from {
        Some(from) => {
            let id: RunId = from
                .strip_prefix("run:")
                .and_then(|id| id.parse().ok())
                .ok_or_else(|| refuse(format!("`{from}` does not name a run")))?;
            let known = match store {
                Some(store) => store.run(&id).map_err(stored)?.is_some(),
                None => false,
            };
            if known {
                Ok(id)
            } else {
                Err(refuse(format!("The run {id} is not in the fact store")))
            }
        }
        None => store
            .map(Store::last_result_run)
            .transpose()
            .map_err(stored)?
            .flatten()
            .ok_or_else(|| refuse("No stored run can be the baseline yet".to_owned())),
    }
}

/// The preview of pointing the baseline `name` from `current` to `target`: a plan with the
/// store row's diff and the command that makes the change.
fn previewed(
    run: &Run,
    writer: &Writer,
    events: &mut dyn EventSink,
    name: &str,
    current: Option<RunId>,
    target: RunId,
    command: &str,
) -> Envelope {
    let before = current.map(|run| format!("{run}\n"));
    let change = PlannedWrite::change(
        &format!("{}/{}", Writer::DIR, Store::FILE),
        &format!("baselines/{name}"),
        before.as_deref(),
        &format!("{target}\n"),
    );
    let plan = RunPlan {
        would_write: vec![change],
        ..RunPlan::default()
    };
    let mut envelope = run.plan_envelope(plan, writer, events, Vec::new());
    envelope.next_actions.push(NextAction {
        priority: 1,
        finding: None,
        kind: ActionKind::Configure,
        summary: format!("Nothing was set; run `{command}` to set the baseline."),
        command: Some(command.to_owned()),
    });
    envelope
}

/// The baseline `name`'s run: its checks and findings, with `run.baseline` naming it.
fn show(run: &Run, store: &Store, name: &str) -> Result<Envelope, CupelError> {
    let (id, baseline) = baseline_run(store, name)?;
    let mut envelope = run.reported(Status::Pass, ExitCode::Pass, baseline.findings);
    envelope.checks = baseline.checks;
    envelope.run.baseline = Some(BaselineRef {
        id,
        name: name.to_owned(),
    });
    Ok(envelope)
}

/// The latest result run compared with `against` (default: the baseline `name`): new findings,
/// then resolved ones, with counts, score deltas, and the reference in `run.baseline`.
fn diff(
    run: &Run,
    store: &Store,
    root: &Path,
    name: &str,
    against: Option<&str>,
) -> Result<Envelope, CupelError> {
    let stored = |error: cupel_store::StoreError| persist::unusable(&error);
    let latest = store.last_result_run().map_err(stored)?.ok_or_else(|| {
        CupelError::bad_argument(
            "--against",
            against,
            "No stored run to compare yet".to_owned(),
            "Run `cupel doctor` or `cupel check` first.".to_owned(),
        )
    })?;
    let current = store
        .run(&latest)
        .map_err(stored)?
        .ok_or_else(|| unset(name))?;
    let reference = against::resolve(store, root, against.unwrap_or("baseline"), name)?;
    let deltas = deltas(&current, &reference.envelope);
    let findings = current
        .findings
        .iter()
        .filter(|finding| deltas.new.contains(&finding.id))
        .cloned()
        .chain(deltas.resolved.iter().cloned())
        .collect();
    let mut envelope = run.reported(Status::Pass, ExitCode::Pass, findings);
    envelope.checks = current.checks;
    deltas.apply(&mut envelope, reference.baseline, &reference.vs);
    Ok(envelope)
}

/// The run the baseline `name` points at, with its stored envelope.
fn baseline_run(store: &Store, name: &str) -> Result<(RunId, Envelope), CupelError> {
    let stored = |error: cupel_store::StoreError| persist::unusable(&error);
    let id = store
        .baseline(name)
        .map_err(stored)?
        .ok_or_else(|| unset(name))?;
    let envelope = store.run(&id).map_err(stored)?.ok_or_else(|| unset(name))?;
    Ok((id, envelope))
}

fn unset(name: &str) -> CupelError {
    CupelError::bad_argument(
        "--name",
        Some(name),
        format!("No baseline named `{name}` is set"),
        "Set one with `cupel baseline set --write`.".to_owned(),
    )
}

fn bad_file(file: &Path, message: &str) -> CupelError {
    let shown = file.display().to_string();
    CupelError::bad_argument(
        "FILE",
        Some(&shown),
        format!("`{shown}` is not a readable baseline: {message}"),
        "Pass a file written by `cupel baseline export`.".to_owned(),
    )
}

/// The usage error for an export the `Writer` refused or could not make.
fn refused(file: &Path, error: &cupel_core::Error) -> CupelError {
    let shown = file.display().to_string();
    let fix = if matches!(error, cupel_core::Error::Write(WriteError::Outside { .. })) {
        "Pass --write to export outside .cupel/, or export to a path under .cupel/."
    } else {
        "Export to a writable path."
    };
    CupelError::bad_argument("FILE", Some(&shown), error.to_string(), fix.to_owned())
}
