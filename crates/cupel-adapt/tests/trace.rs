// ABOUTME: The trace validator (ADR-116) on the real tool: TLC validates the traces the spec.trace Rust fixture's tests
// ABOUTME: record against its spec, exhibiting every construct of the `traces` inventory; and what it does without a spec.
mod support;

use cupel_core::config::{OracleConfig, OracleKind};
use cupel_core::{
    Availability, Cancellation, FactKind, ProduceCtx, RunOracle, Scope, TestResults, TraceStatus,
    Traces,
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

/// TLC's traces of a copy of the spec.trace Rust fixture over `scope`, having checked it wrote
/// nothing in the copy; `None` (reported as skipped) when TLC is not installed.
fn validated(scope: &Scope) -> Option<Traces> {
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
        return None;
    }
    let before = support::tree(root);
    let fact = traces(&ctx, scope);
    assert_eq!(support::tree(root), before, "tool.tlc wrote in the tree");
    Some(fact)
}

/// A divergence of the fixture's `src/lib.rs` at `line`, as the fact serializes it.
fn diverged(
    step: u64,
    action: Option<&str>,
    count: u32,
    previous: Option<u32>,
    line: u32,
) -> serde_json::Value {
    json!({
        "step": step,
        "action": action,
        "state": {"count": count, "done": false},
        "previous": previous.map(|count| json!({"count": count, "done": false})),
        "location": {"file": "src/lib.rs", "line": line, "col": null, "end_line": line,
                     "end_col": null},
    })
}

#[test]
fn tlc_finds_where_each_diverged_trace_leaves_the_spec() {
    let Some(fact) = validated(&Scope::default()) else {
        return;
    };
    assert_eq!(fact.problem, None);
    let divergences: Vec<(String, serde_json::Value)> = fact
        .traces
        .iter()
        .filter(|trace| trace.status == TraceStatus::Diverged)
        .map(|trace| (trace.name.clone(), json!(trace.divergence)))
        .collect();
    assert_eq!(
        divergences,
        [
            (
                "bumps.jsonl".to_owned(),
                diverged(3, Some("Inc"), 3, Some(1), 33)
            ),
            ("restarts.jsonl".to_owned(), diverged(1, None, 5, None, 22)),
        ]
    );
}

#[test]
fn tlc_validates_every_recorded_trace_and_exhibits_every_construct() {
    let Some(fact) = validated(&Scope::default()) else {
        return;
    };
    let value = serde_json::to_value(&fact).unwrap();
    assert_eq!(
        value["spec"],
        json!({"oracle": "counter", "file": "spec/Counter.tla", "lang": "tla+"})
    );
    let verdicts: Vec<(&str, u64, TraceStatus, Option<&str>)> = fact
        .traces
        .iter()
        .map(|trace| {
            (
                trace.name.as_str(),
                trace.steps,
                trace.status,
                trace.detail.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        verdicts,
        [
            ("bumps.jsonl", 3, TraceStatus::Diverged, None),
            ("counts.jsonl", 4, TraceStatus::Conforms, None),
            ("restarts.jsonl", 2, TraceStatus::Diverged, None),
            (
                "torn.jsonl",
                0,
                TraceStatus::Failed,
                Some("line 1: not JSON: EOF while parsing a value at line 1 column 10")
            ),
        ]
    );
    assert_eq!(
        unexhibited(&fact_inventory(FactKind::Traces), &value, &[]),
        []
    );
}

#[test]
fn a_trace_logged_outside_the_scope_is_dropped() {
    let Some(fact) = validated(&Scope::of_files(vec!["tests/trace.rs".to_owned()])) else {
        return;
    };
    let names: Vec<&str> = fact
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
    assert_eq!(
        problems.map(|fact| (
            fact.traces.len(),
            fact.spec.is_some(),
            fact.problem.unwrap_or_default()
        )),
        [
            (
                0,
                false,
                "the run has no oracle: define a spec oracle with `cupel oracle add NAME --kind \
                 spec --spec FILE --lang tla+`"
                    .to_owned()
            ),
            (
                0,
                false,
                "the run's oracle `counter` is not a specification: name a spec oracle with \
                 `--oracle`"
                    .to_owned()
            ),
            (
                0,
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
