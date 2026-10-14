// ABOUTME: Golden envelopes for `cupel survey` on every language's fixture repository (ADR-062), split so
// ABOUTME: each check's results, findings, and rankings are its own golden beside its matrix goldens (ADR-074).
use std::collections::BTreeSet;

use cupel_core::{CheckStatus, Envelope, MetricPer, Severity, Status};
use cupel_fixtures::matrix::Registries;
use cupel_fixtures::{Goldens, copy_repo, redact, run_cupel};
use serde_json::{Value, json};

#[track_caller]
fn assert_golden(name: &str, actual: &Value) {
    if let Err(error) = Goldens::from_env().check(name, actual) {
        panic!("{error}");
    }
}

/// Every check that declares per-target metrics ranks something when it scored, and reports
/// exactly the gate keys it declares.
fn assert_metrics_match_manifests(registries: &Registries, envelope: &Envelope, lang: &str) {
    let rankings = envelope.rankings.as_ref().expect("survey ranks");
    for check in registries.checks.iter() {
        let manifest = check.manifest();
        let declared = |per: fn(MetricPer) -> bool| -> BTreeSet<&str> {
            manifest
                .metrics
                .iter()
                .filter(|metric| per(metric.per))
                .map(|metric| metric.name.as_str())
                .collect()
        };
        let ranked: BTreeSet<&str> = rankings
            .dimensions
            .iter()
            .filter(|d| d.check == manifest.id && !d.entries.is_empty())
            .map(|d| d.metric.as_str())
            .collect();
        let result = envelope
            .checks
            .iter()
            .find(|result| result.id == manifest.id)
            .expect("every check has an entry");
        // A check that scored nothing (static.crap without coverage) measures nothing.
        let scored = result.status != CheckStatus::Skipped && result.score.is_some();
        let per_target = declared(|per| per != MetricPer::Scope);
        assert!(
            !scored || per_target.is_empty() || !ranked.is_empty(),
            "{lang}: {} declares {per_target:?} but ranks nothing",
            manifest.id
        );
        if scored {
            let reported: BTreeSet<&str> = result.metrics.keys().map(String::as_str).collect();
            assert_eq!(
                reported,
                declared(|per| per == MetricPer::Scope),
                "{lang}: {} reports every gate key it declares",
                manifest.id
            );
        }
    }
}

/// What spans every check is computed from the findings, so it is asserted against them
/// rather than kept in a golden every new check would change: the finding totals by severity,
/// and `next_actions` as `cupel_core::next_actions` ranks the findings.
fn assert_totals_follow_from_findings(envelope: &Envelope, lang: &str) {
    assert!(!envelope.truncated, "{lang}");
    assert_eq!(
        envelope.total_findings,
        envelope.findings.len() as u64,
        "{lang}"
    );
    let count = |severity: Severity| -> u64 {
        envelope
            .findings
            .iter()
            .filter(|finding| finding.severity == severity)
            .count() as u64
    };
    let totals = &envelope.summary.findings;
    assert_eq!(
        (totals.fail, totals.warn, totals.info),
        (
            count(Severity::Fail),
            count(Severity::Warn),
            count(Severity::Info)
        ),
        "{lang}"
    );
    assert_eq!(
        serde_json::to_value(&envelope.next_actions).unwrap(),
        serde_json::to_value(cupel_core::next_actions(envelope, "cupel survey")).unwrap(),
        "{lang}"
    );
}

/// The entries of the array at `pointer` in `envelope` whose `key` is `check`.
fn owned_by(envelope: &Value, pointer: &str, key: &str, check: &str) -> Value {
    let entries: Vec<Value> = envelope
        .pointer(pointer)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| entry[key] == check)
        .cloned()
        .collect();
    Value::Array(entries)
}

#[test]
fn survey_matches_its_goldens_for_every_language_fixture() {
    let registries = Registries::builtin();
    let languages = registries.languages.ids();
    assert_eq!(languages.len(), 5, "the five launch languages");
    for lang in languages {
        let dir = tempfile::tempdir().unwrap();
        copy_repo(lang.as_str(), dir.path());

        let outcome = run_cupel(&["survey", "--format", "json"], dir.path());

        assert_eq!(outcome.code, Some(0), "{lang}: {}", outcome.stdout);
        let envelope = outcome.envelope();
        assert_eq!(envelope.status, Status::Pass, "{lang}");
        assert_metrics_match_manifests(&registries, envelope, lang.as_str());
        assert_totals_follow_from_findings(envelope, lang.as_str());
        let mut actual = redact(envelope, &outcome.cwd);
        for check in registries.checks.iter() {
            let id = check.manifest().id.as_str();
            let slice = json!({
                "checks": owned_by(&actual, "/checks", "id", id),
                "findings": owned_by(&actual, "/findings", "check", id),
                "rankings": owned_by(&actual, "/rankings/dimensions", "check", id),
            });
            assert_golden(&format!("{id}/survey-{lang}"), &slice);
        }
        let residual = actual.as_object_mut().unwrap();
        for key in ["checks", "findings", "total_findings", "next_actions"] {
            residual.remove(key);
        }
        residual["summary"]
            .as_object_mut()
            .unwrap()
            .remove("findings");
        residual["rankings"]
            .as_object_mut()
            .unwrap()
            .remove("dimensions");
        assert_golden(&format!("survey/{lang}"), &actual);
    }
}
