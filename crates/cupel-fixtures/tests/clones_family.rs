// ABOUTME: `cupel check --only static.clones` over mixed trees: JavaScript and TypeScript symbols are
// ABOUTME: compared within the `ecma` family, while Go and Python symbols never meet (ADR-051).
use std::collections::BTreeSet;
use std::fs;

use cupel_core::Envelope;
use cupel_fixtures::{copy_repo, run_cupel};

/// `static.clones` over a tree holding the fixture repositories of `langs` side by side.
fn clones(langs: &[&str]) -> Envelope {
    let dir = tempfile::tempdir().unwrap();
    for lang in langs {
        let into = dir.path().join(lang);
        fs::create_dir_all(&into).unwrap();
        copy_repo(lang, &into);
    }
    let outcome = run_cupel(
        &["check", "--only", "static.clones", "--format", "json"],
        dir.path(),
    );
    outcome.envelope().clone()
}

/// The set of languages among each finding's members.
fn member_langs(envelope: &Envelope) -> Vec<BTreeSet<String>> {
    envelope
        .findings
        .iter()
        .map(|finding| {
            let evidence = &finding.evidence.0;
            let members: Vec<&serde_json::Value> = match finding.kind.as_str() {
                "clone.pair" => vec![&evidence["left"], &evidence["right"]],
                _ => evidence["members"].as_array().unwrap().iter().collect(),
            };
            members
                .iter()
                .map(|member| member["lang"].as_str().unwrap().to_owned())
                .collect()
        })
        .collect()
}

#[test]
fn javascript_and_typescript_clones_are_found_across_the_two_languages() {
    let envelope = clones(&["javascript", "typescript"]);
    let langs = member_langs(&envelope);
    let both: BTreeSet<String> = ["javascript", "typescript"].map(str::to_owned).into();
    assert!(langs.contains(&both), "{langs:?}");
    assert_eq!(envelope.checks.len(), 1, "one entry for the family");
    assert!(
        envelope
            .findings
            .iter()
            .all(|f| f.check.as_str() == "static.clones")
    );
}

#[test]
fn go_and_python_clones_stay_within_their_own_language() {
    let envelope = clones(&["go", "python"]);
    let langs = member_langs(&envelope);
    assert!(!langs.is_empty());
    assert!(langs.iter().all(|set| set.len() == 1), "{langs:?}");
}
