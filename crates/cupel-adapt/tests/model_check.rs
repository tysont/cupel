// ABOUTME: The bounded model checker (ADR-117) on the real tool: Kani checks the conc.model-check Rust fixture's
// ABOUTME: harnesses, refuting two with their places and values, writing nothing in the tree, exhibiting the inventory.
mod support;

use cupel_core::{Availability, Cancellation, FactKind, ModelCheckResults, RunStatus, Scope};
use cupel_fixtures::conformance::{fact_inventory, unexhibited};
use serde_json::json;

/// Kani's results on a copy of the fixture over `scope`; `None` (reported as skipped) when Kani
/// is not installed.
fn checked(scope: &Scope) -> Option<ModelCheckResults> {
    let fixture = support::own_fixture("conc.model-check", "rust");
    let root = fixture.path();
    let producer = support::producer(
        support::shared_registry(),
        FactKind::ModelCheckResults,
        "tool.kani",
        "rust",
    );
    let ctx = support::produce_ctx(root, "rust", Cancellation::new());
    if let Availability::MissingTool { name, .. } = producer.availability(&ctx.ctx) {
        eprintln!("SKIPPED: {name} not installed; tool.kani was not run");
        return None;
    }
    let before = support::tree(root);
    let fact = producer.produce(&ctx, scope, &mut Vec::new()).unwrap();
    assert_eq!(support::tree(root), before, "tool.kani wrote in the tree");
    Some(fact.downcast::<ModelCheckResults>().unwrap())
}

/// Each harness as [name, (file, line), status, [[description, function, (file, line), values]]].
fn summary(results: &ModelCheckResults) -> Vec<serde_json::Value> {
    results
        .harnesses
        .iter()
        .map(|harness| {
            let refuted: Vec<serde_json::Value> = harness
                .refuted
                .iter()
                .map(|property| {
                    json!([
                        property.description,
                        property.function,
                        property
                            .location
                            .as_ref()
                            .map(|at| (at.file.clone(), at.line)),
                        property.counterexample.as_ref().map(|c| c.values.clone()),
                    ])
                })
                .collect();
            json!([
                harness.name,
                harness
                    .location
                    .as_ref()
                    .map(|at| (at.file.clone(), at.line)),
                harness.status,
                refuted,
            ])
        })
        .collect()
}

#[test]
fn kani_refutes_the_overflow_and_the_out_of_bounds_read_and_verifies_the_midpoint() {
    let Some(results) = checked(&Scope::default()) else {
        return;
    };
    assert_eq!(results.runs.len(), 1);
    assert_eq!(results.runs[0].status, RunStatus::Ran, "{:?}", results.runs);
    let summary = summary(&results);
    assert_eq!(
        summary,
        [
            json!([
                "proofs::at_stays_in_bounds",
                ["src/lib.rs", 36],
                "failed",
                [[
                    "index out of bounds: the length is less than or equal to the given index",
                    "at",
                    ["src/lib.rs", 13],
                    ["4ul"],
                ]],
            ]),
            json!([
                "proofs::average_never_overflows",
                ["src/lib.rs", 21],
                "failed",
                [[
                    "attempt to add with overflow",
                    "average",
                    ["src/lib.rs", 3],
                    ["128", "128"]
                ]],
            ]),
            json!([
                "proofs::midpoint_lies_between",
                ["src/lib.rs", 28],
                "verified",
                []
            ]),
        ]
    );
    assert_eq!(
        unexhibited(
            &fact_inventory(FactKind::ModelCheckResults),
            &serde_json::to_value(&results).unwrap(),
            &[(
                "undetermined",
                "Kani is undetermined only on constructs it does not support, which no stable \
                 fixture reaches; its reader maps the verdict (unit test)."
            )],
        ),
        []
    );
}

#[test]
fn a_root_without_a_rust_file_in_scope_is_not_checked() {
    let Some(results) = checked(&Scope::of_files(vec!["README.md".to_owned()])) else {
        return;
    };
    assert!(results.runs.is_empty(), "{:?}", results.runs);
    assert!(results.harnesses.is_empty());
}
