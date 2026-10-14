// ABOUTME: Golden envelopes for `cupel survey` on every language's fixture repository (ADR-062): the
// ABOUTME: registered checks' results, findings, metrics, and rankings; they change whenever a check or fixture does.
use std::collections::BTreeSet;

use cupel_core::{CheckStatus, MetricPer, Status};
use cupel_fixtures::matrix::Registries;
use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn survey_matches_its_golden_for_every_language_fixture() {
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
        outcome.assert_golden(&format!("survey/{lang}"));
    }
}
