// ABOUTME: `cupel check --only static.clones` over fixture trees: JavaScript and TypeScript are compared within
// ABOUTME: the `ecma` family, Go and Python never meet (ADR-051), and Go and Java show all three clone types (ADR-136).
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

/// Each pair's clone type with its symbols, and whether it lists renamed texts and edits.
fn typed_pairs(envelope: &Envelope) -> Vec<(u64, String, bool, bool)> {
    envelope
        .findings
        .iter()
        .filter(|finding| finding.kind == "clone.pair")
        .map(|finding| {
            let evidence = &finding.evidence.0;
            let listed = |key: &str| !evidence[key].as_array().unwrap().is_empty();
            (
                evidence["clone_type"].as_u64().unwrap(),
                evidence["left"]["symbol"].as_str().unwrap().to_owned(),
                listed("renamed"),
                listed("edits"),
            )
        })
        .collect()
}

#[test]
fn go_and_java_fixtures_hold_a_clone_of_each_type_with_what_differs() {
    for (lang, exact, renamed, extended) in [
        (
            "go",
            "ledger.runningRefunds",
            "ledger.settleInvoices",
            "ledger.orderLines",
        ),
        (
            "java",
            "ledger.ClonesSettlement.runningRefunds",
            "ledger.ClonesSettlement.settleInvoices",
            "ledger.ClonesSettlement.orderLines",
        ),
    ] {
        let pairs = typed_pairs(&clones(&[lang]));
        for expected in [
            (1, exact.to_owned(), false, false),
            (2, renamed.to_owned(), true, false),
            (3, extended.to_owned(), false, true),
        ] {
            assert!(
                pairs.contains(&expected),
                "{lang}: {expected:?} in {pairs:?}"
            );
        }
    }
}
