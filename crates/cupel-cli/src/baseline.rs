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
    let baseline = Baseline {
        run,
        writer,
        name: name.unwrap_or(DEFAULT_BASELINE),
    };
    match action {
        BaselineArgs::Set { from } => baseline.set(from.as_deref(), events),
        BaselineArgs::Show | BaselineArgs::Diff { .. } if writer.is_recording() => {
            Ok(run.plan_envelope(RunPlan::default(), writer, events, Vec::new()))
        }
        BaselineArgs::Show => show(run, &persist::open(writer)?, baseline.name),
        BaselineArgs::Diff { against } => diff(
            run,
            &persist::open(writer)?,
            writer.root(),
            baseline.name,
            against.as_deref(),
        ),
        BaselineArgs::Export { file } => baseline.export(file, events),
        BaselineArgs::Import { file } => baseline.import(file, events),
    }
}

/// One named baseline, acted on in one run.
struct Baseline<'a> {
    run: &'a Run,
    writer: &'a Writer,
    name: &'a str,
}

impl Baseline<'_> {
    /// The command that addresses this baseline: `cupel baseline`, with `--name` unless it is
    /// the default one.
    fn command(&self) -> String {
        if self.name == DEFAULT_BASELINE {
            "cupel baseline".to_owned()
        } else {
            format!("cupel baseline --name {}", self.name)
        }
    }

    /// `set`: points the baseline at `--from`'s run or the latest result run, or previews that.
    fn set(&self, from: Option<&str>, events: &mut dyn EventSink) -> Result<Envelope, CupelError> {
        if self.writer.is_recording() {
            let store = persist::open_for_reading(self.writer)?;
            let target = chosen(store.as_ref(), from)?;
            let from = from
                .map(|from| format!(" --from {from}"))
                .unwrap_or_default();
            let command = format!("{} set{from} --write", self.command());
            let current = current(store.as_ref(), self.name)?;
            return Ok(self.previewed(events, current, target, &command));
        }
        let store = persist::open(self.writer)?;
        let target = chosen(Some(&store), from)?;
        store
            .set_baseline(self.name, &target)
            .map_err(|error| persist::unusable(&error))?;
        show(self.run, &store, self.name)
    }

    /// `export`: writes the baseline's run to `file`.
    fn export(&self, file: &Path, events: &mut dyn EventSink) -> Result<Envelope, CupelError> {
        let store = persist::open_for_reading(self.writer)?.ok_or_else(|| unset(self.name))?;
        let (id, envelope) = baseline_run(&store, self.name)?;
        let mut json = serde_json::to_string_pretty(&envelope)
            .map_err(|error| bad_file(file, &error.to_string()))?;
        json.push('\n');
        self.writer
            .write(file, json.as_bytes())
            .map_err(|error| refused(file, &error))?;
        if self.writer.is_recording() {
            return Ok(self
                .run
                .plan_envelope(RunPlan::default(), self.writer, events, Vec::new()));
        }
        let mut exported = self.run.reported(Status::Pass, ExitCode::Pass, Vec::new());
        exported.run.baseline = Some(BaselineRef {
            id,
            name: self.name.to_owned(),
        });
        Ok(exported)
    }

    /// `import`: stores the run in `file` and points the baseline at it, or previews that.
    fn import(&self, file: &Path, events: &mut dyn EventSink) -> Result<Envelope, CupelError> {
        let text = std::fs::read_to_string(self.writer.root().join(file))
            .map_err(|error| bad_file(file, &error.to_string()))?;
        let imported: Envelope =
            serde_json::from_str(&text).map_err(|error| bad_file(file, &error.to_string()))?;
        if self.writer.is_recording() {
            let store = persist::open_for_reading(self.writer)?;
            let command = format!("{} import {} --write", self.command(), file.display());
            let current = current(store.as_ref(), self.name)?;
            return Ok(self.previewed(events, current, imported.run.id, &command));
        }
        let stored = |error: cupel_store::StoreError| persist::unusable(&error);
        let store = persist::open(self.writer)?;
        store.record_run(&imported).map_err(stored)?;
        store
            .set_baseline(self.name, &imported.run.id)
            .map_err(stored)?;
        show(self.run, &store, self.name)
    }

    /// The preview of pointing the baseline from `current` to `target`: a plan with the store
    /// row's diff and `command`, which makes the change.
    fn previewed(
        &self,
        events: &mut dyn EventSink,
        current: Option<RunId>,
        target: RunId,
        command: &str,
    ) -> Envelope {
        let before = current.map(|run| format!("{run}\n"));
        let change = PlannedWrite::change(
            &format!("{}/{}", Writer::DIR, Store::FILE),
            &format!("baselines/{}", self.name),
            before.as_deref(),
            &format!("{target}\n"),
        );
        let plan = RunPlan {
            would_write: vec![change],
            ..RunPlan::default()
        };
        let mut envelope = self
            .run
            .plan_envelope(plan, self.writer, events, Vec::new());
        envelope.next_actions.push(NextAction {
            priority: 1,
            finding: None,
            kind: ActionKind::Configure,
            summary: format!("Nothing was set; run `{command}` to set the baseline."),
            command: Some(command.to_owned()),
        });
        envelope
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
