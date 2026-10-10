// ABOUTME: Golden envelopes for the registry-rendered `describe` subjects: every language, the
// ABOUTME: language list, the `symbols` fact, and the support matrix.
use cupel_fixtures::run_cupel;
use cupel_lang::LanguageRegistry;

fn assert_describe_golden(args: &[&str], golden: &str) {
    let dir = tempfile::tempdir().unwrap();
    let mut args = args.to_vec();
    args.extend(["--format", "json"]);

    let outcome = run_cupel(&args, dir.path());

    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    outcome.assert_golden(golden);
}

#[test]
fn describe_languages_matches_its_golden() {
    assert_describe_golden(&["describe", "languages"], "describe/languages");
}

#[test]
fn describe_language_matches_its_golden_for_every_language() {
    for id in LanguageRegistry::builtin().ids() {
        assert_describe_golden(
            &["describe", "language", id.as_str()],
            &format!("describe/language-{id}"),
        );
    }
}

#[test]
fn describe_fact_symbols_matches_its_golden() {
    assert_describe_golden(&["describe", "fact", "symbols"], "describe/fact-symbols");
}

#[test]
fn describe_matrix_matches_its_golden() {
    assert_describe_golden(&["describe", "matrix"], "describe/matrix");
}
