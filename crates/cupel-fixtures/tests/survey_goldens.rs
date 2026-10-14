// ABOUTME: Golden envelopes for `cupel survey` on every language's fixture repository (ADR-062): the
// ABOUTME: registered checks' results, findings, and rankings; they change whenever a check or fixture does.
use cupel_core::Status;
use cupel_fixtures::matrix::Registries;
use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn survey_matches_its_golden_for_every_language_fixture() {
    let languages = Registries::builtin().languages.ids();
    assert_eq!(languages.len(), 5, "the five launch languages");
    for lang in languages {
        let dir = tempfile::tempdir().unwrap();
        copy_repo(lang.as_str(), dir.path());

        let outcome = run_cupel(&["survey", "--format", "json"], dir.path());

        assert_eq!(outcome.code, Some(0), "{lang}: {}", outcome.stdout);
        let envelope = outcome.envelope();
        assert_eq!(envelope.status, Status::Pass, "{lang}");
        let rankings = envelope.rankings.as_ref().expect("survey ranks");
        assert!(
            rankings.dimensions.iter().any(|d| !d.entries.is_empty()),
            "{lang}: some dimension ranks the fixture's symbols"
        );
        outcome.assert_golden(&format!("survey/{lang}"));
    }
}
