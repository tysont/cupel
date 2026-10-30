// ABOUTME: The CLI's access to the fact store: opening it for verbs (failures are usage errors) and the
// ABOUTME: persistence hook that records every run and mirrors the suppressions file (ADR-045 §8, §10).
use std::collections::BTreeMap;

use cupel_core::budget::{TaskDuration, TaskShape};
use cupel_core::{
    CheckId, Envelope, LangId, Measured, Metric, MutationOutcomes, MutationPhase, ProducerId,
    SuitePhase, SuiteReuse, Suppressions, TestResults, Writer,
};
use cupel_store::{Store, StoreError};

use crate::error::CupelError;
use crate::run::Executions;

/// The subject and argument of a store failure in a verb.
const STORE: &str = ".cupel/cupel.db";

/// The store, opened for reading and writing.
pub fn open(writer: &Writer) -> Result<Store, CupelError> {
    Store::open(writer).map_err(|error| unusable(&error))
}

/// The store for reading: opened read-only by a dry run, which creates nothing, and normally
/// otherwise. `None` only for a dry run in a repository without a store.
pub fn open_for_reading(writer: &Writer) -> Result<Option<Store>, CupelError> {
    if writer.is_recording() {
        Store::open_existing(writer).map_err(|error| unusable(&error))
    } else {
        open(writer).map(Some)
    }
}

/// The usage error for a store that cannot be used.
pub fn unusable(error: &StoreError) -> CupelError {
    CupelError::bad_argument(
        STORE,
        None,
        format!("Cannot use the fact store: {error}"),
        "Run `cupel doctor`; .cupel/ must be a writable directory.".to_owned(),
    )
}

/// Records `envelope` in the store with what it executed (its test runs, ADR-076 §7, its
/// mutation outcomes, ADR-080 §10, its durations, ADR-086 §2, with the mutation engine's phases
/// and the suite joints' among them, ADR-150 §2 and ADR-155 §3, and its checks' per-symbol measurements, which a later comparison
/// across symbol matches reads, ADR-131) and mirrors
/// `suppressions` (`.cupel/suppressions.toml`, when it loads) into it. The error is for one
/// stderr line; it never changes the run's outcome (ADR-045 §8).
pub fn record(
    writer: &Writer,
    envelope: &Envelope,
    executed: &Executions,
    suppressions: Option<&Suppressions>,
) -> Result<(), String> {
    let store = Store::open(writer).map_err(|error| error.to_string())?;
    store
        .record_run(envelope)
        .map_err(|error| error.to_string())?;
    let mut durations = executed.durations.clone();
    for (lang, results) in &executed.test_results {
        store
            .record_test_results(&envelope.run.id, lang, results)
            .map_err(|error| error.to_string())?;
        durations.extend(suite_phase_durations(lang, results));
    }
    for (lang, outcomes) in &executed.mutation_outcomes {
        store
            .record_mutants(&envelope.run.id, lang, outcomes)
            .map_err(|error| error.to_string())?;
        durations.extend(phase_durations(lang, outcomes));
    }
    store
        .record_durations(&envelope.run.id, &durations)
        .map_err(|error| error.to_string())?;
    for (check, metrics) in symbol_metrics(&executed.measurements) {
        store
            .record_metrics(&envelope.run.id, &check, &metrics)
            .map_err(|error| error.to_string())?;
    }
    if let Some(suppressions) = suppressions {
        store
            .replace_suppressions(suppressions.entries())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Each phase of `outcomes`' production as a duration of its own, `<engine>.<phase>`, with the
/// phase's total (ADR-150 §2); none for a phase the production did not go through.
fn phase_durations(lang: &LangId, outcomes: &MutationOutcomes) -> Vec<TaskDuration> {
    let rows: Vec<(&str, u64)> = outcomes
        .phases
        .iter()
        .map(|row| (row.phase.as_str(), row.duration_ms))
        .collect();
    let names = MutationPhase::ALL.map(MutationPhase::as_str);
    totals(lang, outcomes.engine.as_str(), &names, &rows)
}

/// Each phase of the joint suite run that made `results` as a duration of its own,
/// `<joint>.<phase>`, with the phase's total over its roots (ADR-155 §3); none for a phase the
/// run did not go through, or results no timed joint made.
fn suite_phase_durations(lang: &LangId, results: &TestResults) -> Vec<TaskDuration> {
    let Some(SuiteReuse {
        joint: Some(joint),
        phases,
        ..
    }) = &results.reuse
    else {
        return Vec::new();
    };
    let rows: Vec<(&str, u64)> = phases
        .iter()
        .map(|row| (row.phase.as_str(), row.duration_ms))
        .collect();
    let names = SuitePhase::ALL.map(SuitePhase::as_str);
    totals(lang, joint.as_str(), &names, &rows)
}

/// For each phase of `names` that some row of `rows` names, in that order, a duration
/// `<producer>.<phase>` holding the total of its rows.
fn totals(
    lang: &LangId,
    producer: &str,
    names: &[&str],
    rows: &[(&str, u64)],
) -> Vec<TaskDuration> {
    names
        .iter()
        .filter_map(|name| {
            let times: Vec<u64> = rows
                .iter()
                .filter(|(phase, _)| phase == name)
                .map(|(_, ms)| *ms)
                .collect();
            (!times.is_empty()).then(|| TaskDuration {
                shape: TaskShape::producer(
                    &ProducerId::from(format!("{producer}.{name}")),
                    lang,
                    false,
                ),
                duration_ms: times.iter().sum(),
            })
        })
        .collect()
}

/// The measurements of symbols among `measured`, grouped by check in measured order.
fn symbol_metrics(measured: &[Measured]) -> BTreeMap<CheckId, Vec<Metric>> {
    let mut metrics: BTreeMap<CheckId, Vec<Metric>> = BTreeMap::new();
    for each in measured {
        if let Some(symbol) = &each.measurement.target.symbol {
            metrics.entry(each.check.clone()).or_default().push(Metric {
                symbol: Some(symbol.clone()),
                key: each.measurement.metric.clone(),
                value: each.measurement.value,
            });
        }
    }
    metrics
}
