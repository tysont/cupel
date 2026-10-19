// ABOUTME: `--against baseline|REF|run:ID`: resolving the reference run from the fact store and git, and
// ABOUTME: recording deltas against it on an envelope; `check` and `survey` compare this way (ADR-046).
use std::path::Path;

use cupel_core::{BaselineRef, Envelope, RunId, Writer, deltas};
use cupel_store::Store;

use crate::error::CupelError;
use cupel_core::git::Git;

use crate::persist;

/// The baseline `--against` names when it is absent or says `baseline` without `--name`.
pub const DEFAULT_BASELINE: &str = "default";

/// A run to compare with, and how deltas name it.
pub struct Reference {
    pub envelope: Envelope,
    /// What `run.baseline` records.
    pub baseline: BaselineRef,
    /// What `checks[].delta.vs` records: `baseline`, `run:<id>`, or the ref as written.
    pub vs: String,
}

/// The run `spec` names: `baseline` (the baseline `name`), `run:<id>`, or a git ref resolved to
/// the latest stored result run at its commit. Anything unknown is a usage error on `--against`.
/// The latest stored result run made at the commit the git ref `spec` names.
fn run_at(
    store: &Store,
    root: &Path,
    spec: &str,
    refuse: &dyn Fn(String, &str) -> CupelError,
) -> Result<RunId, CupelError> {
    let commit = Git::open(root)
        .and_then(|git| git.commit(spec).ok())
        .ok_or_else(|| {
            refuse(
                format!("git cannot resolve `{spec}` to a commit"),
                "Pass --against baseline, run:ID, or a git ref that names a commit.",
            )
        })?;
    store
        .last_result_run_at(&commit)
        .map_err(|error| persist::unusable(&error))?
        .ok_or_else(|| {
            refuse(
                format!("No stored run was made at `{spec}` ({commit})"),
                "Compare with a commit Cupel has run at, or pass --against run:ID.",
            )
        })
}

pub fn resolve(
    store: &Store,
    root: &Path,
    spec: &str,
    name: &str,
) -> Result<Reference, CupelError> {
    let refuse = |title: String, fix: &str| {
        CupelError::bad_argument("--against", Some(spec), title, fix.to_owned())
    };
    let stored = |error: cupel_store::StoreError| persist::unusable(&error);
    let (run, baseline_name, vs) = if spec == "baseline" {
        let run = store.baseline(name).map_err(stored)?.ok_or_else(|| {
            refuse(
                format!("No baseline named `{name}` is set"),
                "Set one with `cupel baseline set --write`, or pass --against run:ID or a git ref.",
            )
        })?;
        (run, name.to_owned(), "baseline".to_owned())
    } else if let Some(id) = spec.strip_prefix("run:") {
        let run: RunId = id.parse().map_err(|_| {
            refuse(
                format!("`{spec}` does not name a run"),
                "Pass --against run:<run ID>, with the ID from an envelope's run.id.",
            )
        })?;
        (run, spec.to_owned(), spec.to_owned())
    } else {
        let run = run_at(store, root, spec, &refuse)?;
        (run, spec.to_owned(), spec.to_owned())
    };
    let envelope = store.run(&run).map_err(stored)?.ok_or_else(|| {
        refuse(
            format!("The run {run} is not in the fact store"),
            "Pass --against a run this repository's fact store holds.",
        )
    })?;
    Ok(Reference {
        envelope,
        baseline: BaselineRef {
            id: run,
            name: baseline_name,
        },
        vs,
    })
}

/// Records on `envelope` its deltas against `against`, or against the default baseline when
/// `against` is absent and that baseline is set; otherwise leaves it alone (ADR-046 §5–6).
/// Returns the run compared with, if any.
pub fn compare(
    envelope: &mut Envelope,
    writer: &Writer,
    against: Option<&str>,
) -> Result<Option<Envelope>, CupelError> {
    let Some(store) = persist::open_for_reading(writer)? else {
        return Ok(None);
    };
    if against.is_none()
        && store
            .baseline(DEFAULT_BASELINE)
            .map_err(|error| persist::unusable(&error))?
            .is_none()
    {
        return Ok(None);
    }
    let spec = against.unwrap_or("baseline");
    let reference = resolve(&store, writer.root(), spec, DEFAULT_BASELINE)?;
    deltas(envelope, &reference.envelope).apply(envelope, reference.baseline, &reference.vs);
    Ok(Some(reference.envelope))
}

#[cfg(test)]
mod tests {
    use cupel_core::{CheckId, ExitCode, Finding, FindingId, Severity, Status};

    use super::*;
    use crate::problem::Problem;
    use crate::run::Run;

    fn finding(subject: &str) -> Finding {
        Problem {
            check: "doctor",
            kind: "doctor.tool-missing",
            subject: subject.to_owned(),
            severity: Severity::Warn,
            title: subject.to_owned(),
            detail: None,
            evidence: serde_json::Value::Null,
            action: cupel_core::ActionKind::InstallTool,
            fix: "Install it.".to_owned(),
        }
        .finding("cupel doctor")
    }

    fn envelope(subjects: &[&str]) -> Envelope {
        Run::start(vec!["check".to_owned()]).reported(
            Status::Pass,
            ExitCode::Pass,
            subjects.iter().map(|subject| finding(subject)).collect(),
        )
    }

    fn id(subject: &str) -> FindingId {
        FindingId::for_subject(&CheckId::from("doctor"), "doctor.tool-missing", subject)
    }

    #[test]
    fn without_against_or_a_default_baseline_nothing_is_compared() {
        let dir = tempfile::tempdir().unwrap();
        let writer = Writer::new(dir.path(), false).unwrap();
        let mut current = envelope(&["a"]);
        let before = current.clone();
        compare(&mut current, &writer, None).unwrap();
        assert_eq!(current, before);
    }

    /// A fresh store, its directory and writer, holding the recorded `run`.
    fn store_with(run: &Envelope) -> (tempfile::TempDir, Writer, Store) {
        let dir = tempfile::tempdir().unwrap();
        let writer = Writer::new(dir.path(), false).unwrap();
        let store = Store::open(&writer).unwrap();
        store.record_run(run).unwrap();
        (dir, writer, store)
    }

    #[test]
    fn the_default_baseline_is_compared_when_set() {
        let baseline = envelope(&["a", "b"]);
        let (_dir, writer, store) = store_with(&baseline);
        store
            .set_baseline(DEFAULT_BASELINE, &baseline.run.id)
            .unwrap();
        let mut current = envelope(&["b", "c"]);

        compare(&mut current, &writer, None).unwrap();

        let new: Vec<bool> = current
            .findings
            .iter()
            .map(|f| f.new_since_baseline)
            .collect();
        assert_eq!(new, [false, true]);
        assert_eq!(current.findings[1].id, id("c"));
        assert_eq!((current.summary.new, current.summary.resolved), (1, 1));
        assert_eq!(
            current.run.baseline,
            Some(BaselineRef {
                id: baseline.run.id,
                name: DEFAULT_BASELINE.to_owned(),
            })
        );
    }

    #[test]
    fn an_explicit_run_is_compared_and_an_unknown_one_is_a_usage_error() {
        let reference = envelope(&["a"]);
        let (_dir, writer, _store) = store_with(&reference);
        let spec = format!("run:{}", reference.run.id);
        let mut current = envelope(&["a"]);
        compare(&mut current, &writer, Some(&spec)).unwrap();
        assert_eq!(current.run.baseline.map(|b| b.name), Some(spec));

        let error = compare(&mut envelope(&[]), &writer, Some("run:nonsense")).unwrap_err();
        assert_eq!(
            (error.exit_code(), error.to_string()),
            (
                ExitCode::Usage,
                "`run:nonsense` does not name a run".to_owned()
            )
        );
    }
}
