// ABOUTME: The trace validator (ADR-116) on the real tool: TLC validates the traces the spec.trace Rust fixture's tests
// ABOUTME: record against its spec, exhibiting every construct of the `traces` inventory; and what it does without a spec.
mod support;

use cupel_core::config::{OracleConfig, OracleKind};
use cupel_core::{
    Availability, Cancellation, FactKind, ProduceCtx, RunOracle, Scope, TestResults, Traces,
};
use cupel_fixtures::conformance::{fact_inventory, unexhibited};
use serde_json::json;

/// A spec oracle named `counter`, in `lang`, for the fixture's `spec/Counter.tla`.
fn oracle(kind: OracleKind, lang: &str) -> RunOracle {
    RunOracle {
        name: "counter".to_owned(),
        definition: OracleConfig {
            kind,
            reference: None,
            cmd: None,
            spec: Some("spec/Counter.tla".to_owned()),
            lang: Some(lang.to_owned()),
            inputs: Vec::new(),
        },
    }
}

fn traces(ctx: &ProduceCtx, scope: &Scope) -> Traces {
    support::producer(
        support::shared_registry(),
        FactKind::Traces,
        "tool.tlc",
        "rust",
    )
    .produce(ctx, scope, &mut Vec::new())
    .unwrap()
    .downcast::<Traces>()
    .unwrap()
}

/// The context of producing in `root` with the run's `oracle` and, when `baseline`, the
/// fixture's green baseline from `cargo test`.
fn context(root: &std::path::Path, oracle: Option<RunOracle>, baseline: bool) -> ProduceCtx {
    let mut ctx = support::produce_ctx(root, "rust", Cancellation::new());
    ctx.runners = support::shared_registry().runners();
    ctx.oracle = oracle;
    if baseline {
        let results = support::runner(support::shared_registry(), "tool.cargo-test", "rust")
            .produce(&ctx, &Scope::default(), &mut Vec::new())
            .unwrap();
        assert!(results.downcast_ref::<TestResults>().unwrap().is_green());
        ctx.upstream.insert(results.share());
    }
    ctx
}

#[test]
fn tlc_validates_each_recorded_trace_and_exhibits_every_construct() {
    let fixture = support::own_fixture("spec.trace", "rust");
    let root = fixture.path();
    let ctx = context(root, Some(oracle(OracleKind::Spec, "tla+")), true);
    let producer = support::producer(
        support::shared_registry(),
        FactKind::Traces,
        "tool.tlc",
        "rust",
    );
    if let Availability::MissingTool { name, .. } = producer.availability(&ctx.ctx) {
        eprintln!("SKIPPED: {name} not installed; tool.tlc was not run");
        return;
    }
    let before = support::tree(root);

    let fact = traces(&ctx, &Scope::default());

    assert_eq!(support::tree(root), before, "tool.tlc wrote in the tree");
    assert_eq!(fact.problem, None);
    let value = serde_json::to_value(&fact).unwrap();
    assert_eq!(
        value["spec"],
        json!({"oracle": "counter", "file": "spec/Counter.tla", "lang": "tla+"})
    );
    assert_eq!(
        value["traces"],
        json!([
            {
                "name": "bumps.jsonl", "steps": 3, "status": "diverged", "detail": null,
                "divergence": {
                    "step": 3,
                    "action": "Inc",
                    "state": {"count": 3, "done": false},
                    "previous": {"count": 1, "done": false},
                    "location": {"file": "src/lib.rs", "line": 33, "col": null,
                                 "end_line": 33, "end_col": null},
                },
            },
            {"name": "counts.jsonl", "steps": 4, "status": "conforms", "detail": null,
             "divergence": null},
            {
                "name": "restarts.jsonl", "steps": 2, "status": "diverged", "detail": null,
                "divergence": {
                    "step": 1,
                    "action": null,
                    "state": {"count": 5, "done": false},
                    "previous": null,
                    "location": {"file": "src/lib.rs", "line": 22, "col": null,
                                 "end_line": 22, "end_col": null},
                },
            },
            {
                "name": "torn.jsonl", "steps": 0, "status": "failed",
                "detail": "line 1: not JSON: EOF while parsing a value at line 1 column 10",
                "divergence": null,
            },
        ])
    );
    assert_eq!(
        unexhibited(&fact_inventory(FactKind::Traces), &value, &[]),
        []
    );

    let elsewhere = traces(&ctx, &Scope::of_files(vec!["tests/trace.rs".to_owned()]));
    let names: Vec<&str> = elsewhere
        .traces
        .iter()
        .map(|trace| trace.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["torn.jsonl"],
        "only the trace placed nowhere is kept"
    );
}

#[test]
fn without_a_tla_spec_oracle_nothing_is_recorded_and_the_problem_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let problems = [
        None,
        Some(oracle(OracleKind::Ref, "tla+")),
        Some(oracle(OracleKind::Spec, "quint")),
    ]
    .map(|oracle| traces(&context(dir.path(), oracle, false), &Scope::default()));
    for fact in &problems {
        assert!(fact.traces.is_empty());
    }
    assert_eq!(
        problems.map(|fact| (fact.spec.is_some(), fact.problem.unwrap_or_default())),
        [
            (
                false,
                "the run has no oracle: define a spec oracle with `cupel oracle add NAME --kind \
                 spec --spec FILE --lang tla+`"
                    .to_owned()
            ),
            (
                false,
                "the run's oracle `counter` is not a specification: name a spec oracle with \
                 `--oracle`"
                    .to_owned()
            ),
            (
                false,
                "the run's oracle `counter` is a quint specification, which tool.tlc does not \
                 read"
                    .to_owned()
            ),
        ]
    );
    let unrun = traces(
        &context(dir.path(), Some(oracle(OracleKind::Spec, "tla+")), false),
        &Scope::default(),
    );
    assert_eq!(
        unrun.problem.as_deref(),
        Some("no test run to record traces from")
    );
    assert_eq!(support::tree(dir.path()), Vec::<String>::new());
}
