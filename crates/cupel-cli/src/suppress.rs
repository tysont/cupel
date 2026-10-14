// ABOUTME: The `suppress` verb and suppressions in runs: writes `.cupel/suppressions.toml`, mirrors it
// ABOUTME: into the store, and applies it to finished runs with expiry by date or git ref (ADR-047).
use std::io::ErrorKind;

use std::path::Path;

use cupel_core::git::Git;
use cupel_core::{
    Envelope, ExitCode, FindingId, Severity, Status, Suppression, Suppressions, Until, Writer,
    apply_suppressions,
};

use crate::error::CupelError;
use crate::persist;
use crate::run::Run;

/// How the suppressions file is named in errors.
const FILE: &str = ".cupel/suppressions.toml";

/// Suppresses `finding` for `reason` until `until`: the finding must be one the store has
/// recorded. The file is written through `writer`, so a dry run records its diff instead; the
/// store mirror is updated only for a real run. The envelope carries the finding, suppressed.
pub fn run(
    run: &Run,
    writer: &Writer,
    finding: &str,
    reason: &str,
    until: Option<&str>,
) -> Result<Envelope, CupelError> {
    let unknown = || {
        CupelError::bad_argument(
            "FINDING",
            Some(finding),
            format!("No stored run produced the finding `{finding}`"),
            "Suppress a finding ID (fnd_…) from an envelope Cupel produced in this repository."
                .to_owned(),
        )
    };
    let id: FindingId = finding.parse().map_err(|_| unknown())?;
    let until = until
        .map(|until| {
            until.parse::<Until>().map_err(|error| {
                CupelError::bad_argument(
                    "--until",
                    Some(until),
                    error.to_string(),
                    "Pass --until a date as YYYY-MM-DD or a git ref.".to_owned(),
                )
            })
        })
        .transpose()?;
    let store = persist::open_for_reading(writer)?;
    let (mut found, _) = store
        .as_ref()
        .map(|store| store.finding(&id))
        .transpose()
        .map_err(|error| persist::unusable(&error))?
        .flatten()
        .ok_or_else(unknown)?;
    found.suppressed = true;
    found.severity = Severity::Info;
    let envelope = run.reported(Status::Pass, ExitCode::Pass, vec![found]);
    let mut suppressions = load(writer)?;
    suppressions.upsert(Suppression {
        finding: id,
        reason: reason.to_owned(),
        until,
        created: today(&envelope),
    });
    writer.write(
        &writer.cupel_dir().join(Suppressions::FILE),
        suppressions.to_toml().as_bytes(),
    )?;
    if let Some(store) = store.filter(|_| !writer.is_recording()) {
        store
            .replace_suppressions(suppressions.entries())
            .map_err(|error| persist::unusable(&error))?;
    }
    let mut envelope = envelope;
    envelope.summary.suppressed = 1;
    Ok(envelope)
}

/// The repository's suppressions; none when the file (or `.cupel/` itself) does not exist.
pub fn load(writer: &Writer) -> Result<Suppressions, CupelError> {
    let path = writer.cupel_dir().join(Suppressions::FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if matches!(error.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            return Ok(Suppressions::default());
        }
        Err(error) => return Err(malformed(&error.to_string())),
    };
    Suppressions::parse(&text).map_err(|error| malformed(&error.to_string()))
}

fn malformed(message: &str) -> CupelError {
    CupelError::bad_argument(
        FILE,
        None,
        format!("{FILE} cannot be read: {message}"),
        "Fix or remove the entry; each [[suppression]] needs finding, reason, and created, and \
         may have until (YYYY-MM-DD or a git ref)."
            .to_owned(),
    )
}

/// Applies the repository's suppressions to a finished run's envelope (ADR-047 §4–6): a date
/// expires after its day, a ref once `HEAD` contains it; an exit-3 envelope with no fail
/// finding left passes.
pub fn apply(envelope: &mut Envelope, writer: &Writer) -> Result<(), CupelError> {
    let suppressions = load(writer)?;
    if suppressions.entries().is_empty() {
        return Ok(());
    }
    let today = today(envelope);
    apply_suppressions(envelope, suppressions.entries(), |until| match until {
        Until::Date(_) => until.date_passed(&today),
        Until::Ref(reference) => head_contains(writer.root(), reference),
    });
    let fails = envelope
        .findings
        .iter()
        .any(|finding| finding.severity == Severity::Fail);
    if envelope.exit_code == ExitCode::Environment && !fails {
        envelope.status = Status::Pass;
        envelope.exit_code = ExitCode::Pass;
    }
    Ok(())
}

/// The run's start date, `YYYY-MM-DD` in UTC.
fn today(envelope: &Envelope) -> String {
    envelope
        .run
        .started
        .get(..10)
        .unwrap_or_default()
        .to_owned()
}

/// Whether `HEAD` contains the commit `reference` names: it is `HEAD` or an ancestor of it.
fn head_contains(root: &Path, reference: &str) -> bool {
    let Some(git) = Git::open(root) else {
        return false;
    };
    let Ok(commit) = git.commit(reference) else {
        return false;
    };
    git.merge_base(&commit, "HEAD")
        .is_ok_and(|base| base == commit)
}
