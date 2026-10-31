// ABOUTME: The deductive verifier (ADR-118) on the real tool: Dafny verifies the spec.prove fixture's proofs in a Go root,
// ABOUTME: one failed and two open obligations at their places, writing nothing in the tree, exhibiting the inventory.
mod support;

use cupel_core::{Availability, Cancellation, FactKind, ProofResults, RunStatus, Scope};
use cupel_fixtures::conformance::{fact_inventory, unexhibited};
use cupel_fixtures::own_tmpdir::ran_in_own_tmpdir;
use serde_json::json;

/// Each obligation as [status, line, message, [related lines]].
fn summary(fact: &ProofResults) -> Vec<serde_json::Value> {
    fact.obligations
        .iter()
        .map(|obligation| {
            json!([
                obligation.status,
                obligation.location.line,
                obligation.message,
                obligation
                    .related
                    .iter()
                    .map(|related| related.location.line)
                    .collect::<Vec<_>>(),
            ])
        })
        .collect()
}

/// What Dafny reports on the fixture's proof: the unproved postcondition and both assumptions, each
/// as `[state, line, message, related lines]`.
fn dafny_findings() -> [serde_json::Value; 3] {
    [
        json!([
            "failed",
            24,
            "a postcondition could not be proved on this return path",
            [23]
        ]),
        json!([
            "open",
            31,
            "assume statement has no {:axiom} annotation",
            []
        ]),
        json!([
            "open",
            35,
            "This ensures clause is part of a bodyless method. Add the {:axiom} attribute to \
                 it or the enclosing method to suppress this warning",
            []
        ]),
    ]
}

/// Dafny keeps a registration file in the temporary directory, so the test runs in one of its own.
#[test]
fn dafny_reports_the_unproved_postcondition_and_both_assumptions() {
    if ran_in_own_tmpdir("dafny_reports_the_unproved_postcondition_and_both_assumptions") {
        return;
    }
    let (fixture, kind) = (
        support::own_fixture("spec.prove", "go"),
        FactKind::ProofResults,
    );
    let root = fixture.path();
    let producer = support::producer(support::shared_registry(), kind, "tool.dafny", "go");
    let ctx = support::produce_ctx(root, "go", Cancellation::new());
    if let Availability::MissingTool { name, .. } = producer.availability(&ctx.ctx) {
        eprintln!("SKIPPED: {name} not installed; tool.dafny was not run");
        return;
    }
    let before = support::tree(root);

    let fact = producer
        .produce(&ctx, &Scope::default(), &mut Vec::new())
        .unwrap()
        .downcast::<ProofResults>()
        .unwrap();

    assert_eq!(support::tree(root), before, "tool.dafny wrote in the tree");
    assert_eq!(fact.runs.len(), 1);
    assert_eq!(fact.runs[0].status, RunStatus::Ran, "{:?}", fact.runs);
    assert_eq!(fact.files, ["proofs/tally.dfy"]);
    assert_eq!(fact.verified, 3);
    assert_eq!(summary(&fact), dafny_findings());
    assert_eq!(
        unexhibited(
            &fact_inventory(FactKind::ProofResults),
            &serde_json::to_value(&fact).unwrap(),
            &[],
        ),
        []
    );
}

#[test]
fn a_root_without_proof_files_is_not_run() {
    let fixture = support::own_fixture("spec.prove", "go");
    std::fs::remove_dir_all(fixture.path().join("proofs")).unwrap();
    let producer = support::producer(
        support::shared_registry(),
        FactKind::ProofResults,
        "tool.dafny",
        "go",
    );
    let ctx = support::produce_ctx(fixture.path(), "go", Cancellation::new());
    let fact = producer
        .produce(&ctx, &Scope::default(), &mut Vec::new())
        .unwrap()
        .downcast::<ProofResults>()
        .unwrap();
    assert!(fact.runs.is_empty() && fact.files.is_empty(), "{fact:?}");
}
