// ABOUTME: `--against baseline|REF|run:ID`: resolving the reference run from the fact store and git, and recording
// ABOUTME: deltas against it, across the run's symbol matches, on an envelope (ADR-046, ADR-131).
use std::collections::BTreeSet;
use std::path::Path;

use cupel_core::{
    BaselineRef, CheckId, Envelope, Matching, MetricPer, RunId, SymbolMetrics, Writer,
    deltas_matched,
};
use cupel_store::Store;

use crate::error::CupelError;
use cupel_core::git::Git;

use crate::persist;
use crate::run::Run;

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
/// Findings and per-symbol metrics are compared across the symbol matches `run` kept, with the
/// reference run's stored per-symbol metrics (ADR-131).
pub fn compare(
    envelope: &mut Envelope,
    writer: &Writer,
    against: Option<&str>,
    run: &Run,
) -> Result<(), CupelError> {
    let Some(store) = persist::open_for_reading(writer)? else {
        return Ok(());
    };
    if against.is_none()
        && store
            .baseline(DEFAULT_BASELINE)
            .map_err(|error| persist::unusable(&error))?
            .is_none()
    {
        return Ok(());
    }
    let spec = against.unwrap_or("baseline");
    let reference = resolve(&store, writer.root(), spec, DEFAULT_BASELINE)?;
    let mut stored = SymbolMetrics::default();
    for (check, metric) in store
        .symbol_metrics(&reference.baseline.id)
        .map_err(|error| persist::unusable(&error))?
    {
        if let Some(symbol) = metric.symbol {
            stored.insert(&check, &metric.key, symbol, metric.value);
        }
    }
    let deltas = run.kept(|kept| {
        let matching = Matching {
            matches: &kept.matches,
            current: SymbolMetrics::of(&kept.measurements),
            reference: stored,
            means: means(),
        };
        deltas_matched(envelope, &reference.envelope, &matching)
    });
    deltas.apply(envelope, reference.baseline, &reference.vs);
    Ok(())
}

/// The `(check, metric)` pairs of every per-symbol metric a registered check averages.
fn means() -> BTreeSet<(CheckId, String)> {
    crate::catalog::checks()
        .iter()
        .flat_map(|check| {
            let manifest = check.manifest();
            manifest
                .metrics
                .iter()
                .filter(|metric| metric.per != MetricPer::Scope && check.averages(&metric.name))
                .map(|metric| (manifest.id.clone(), metric.name.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use cupel_core::matching::{Matched, SymbolMatch};
    use cupel_core::syntax::SymbolKey;
    use cupel_core::{
        CheckId, ExitCode, Finding, FindingId, LangId, Measured, Measurement, Metric, Severity,
        Status, Symbol, Target,
    };

    use super::*;
    use crate::problem::Problem;
    use crate::run::{Executions, Run};

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
        compare(&mut current, &writer, None, &Run::start(Vec::new())).unwrap();
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

        compare(&mut current, &writer, None, &Run::start(Vec::new())).unwrap();

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

    /// A `complexity.high` finding about the symbol `hash`, from a scored `static.complexity`.
    fn complex(hash: &str) -> Envelope {
        let check = CheckId::from("static.complexity");
        let mut finding = finding(hash);
        finding.check = check.clone();
        finding.kind = "complexity.high".to_owned();
        let hash: cupel_core::SymbolHash = hash.parse().unwrap();
        finding.id = FindingId::compute(&check, "complexity.high", &hash, None);
        finding.symbol = Some(Symbol {
            display: hash.to_string(),
            hash,
            lang: LangId::from("rust"),
        });
        let mut envelope = Run::start(vec!["check".to_owned()]).reported(
            Status::Pass,
            ExitCode::Pass,
            vec![finding],
        );
        envelope.checks = vec![
            serde_json::from_value(serde_json::json!({
                "id": "static.complexity", "status": "pass", "tier": 1, "cost": "quick",
                "score": {"value": 12.0, "unit": "cyclomatic_max", "n": 1},
                "delta": null, "findings": 1, "duration_ms": 1, "skipped": null, "oracle": null
            }))
            .unwrap(),
        ];
        envelope
    }

    fn member(hash: &str) -> Matched {
        Matched {
            key: SymbolKey {
                file: "src/lib.rs".to_owned(),
                line: 1,
                col: 1,
            },
            symbol: Symbol {
                display: hash.to_owned(),
                hash: hash.parse().unwrap(),
                lang: LangId::from("rust"),
            },
            nodes: 50,
        }
    }

    #[test]
    fn a_finding_on_a_renamed_symbol_is_carried_and_its_metrics_compared_across_the_match() {
        const OLD: &str = "sh_00000000000000a1";
        const NEW: &str = "sh_00000000000000b2";
        let reference = complex(OLD);
        let (_dir, writer, store) = store_with(&reference);
        let check = CheckId::from("static.complexity");
        let stored = Metric {
            symbol: Some(OLD.parse().unwrap()),
            key: "cyclomatic".to_owned(),
            value: 12.0,
        };
        store
            .record_metrics(&reference.run.id, &check, &[stored])
            .unwrap();
        let run = Run::start(vec!["check".to_owned()]);
        run.keep(Executions {
            measurements: vec![Measured {
                check,
                lang: LangId::from("rust"),
                measurement: Measurement {
                    metric: "cyclomatic".to_owned(),
                    value: 9.0,
                    target: Target {
                        display: NEW.to_owned(),
                        symbol: Some(NEW.parse().unwrap()),
                        location: None,
                    },
                },
            }],
            matches: vec![SymbolMatch {
                base: vec![member(OLD)],
                head: vec![member(NEW)],
                similarity: 0.9,
            }],
            ..Executions::default()
        });
        let mut current = complex(NEW);
        let spec = format!("run:{}", reference.run.id);

        compare(&mut current, &writer, Some(&spec), &run).unwrap();

        assert_eq!((current.summary.new, current.summary.resolved), (0, 0));
        assert!(!current.findings[0].new_since_baseline);
        let delta = current.checks[0].delta.as_ref().unwrap();
        assert_eq!(
            serde_json::to_value(&delta.sets[0].metrics).unwrap(),
            serde_json::json!({"cyclomatic": {"base": 12.0, "head": 9.0}})
        );
    }

    #[test]
    fn an_explicit_run_is_compared_and_an_unknown_one_is_a_usage_error() {
        let reference = envelope(&["a"]);
        let (_dir, writer, _store) = store_with(&reference);
        let spec = format!("run:{}", reference.run.id);
        let mut current = envelope(&["a"]);
        compare(&mut current, &writer, Some(&spec), &Run::start(Vec::new())).unwrap();
        assert_eq!(current.run.baseline.map(|b| b.name), Some(spec));

        let error = compare(
            &mut envelope(&[]),
            &writer,
            Some("run:nonsense"),
            &Run::start(Vec::new()),
        )
        .unwrap_err();
        assert_eq!(
            (error.exit_code(), error.to_string()),
            (
                ExitCode::Usage,
                "`run:nonsense` does not name a run".to_owned()
            )
        );
    }
}
