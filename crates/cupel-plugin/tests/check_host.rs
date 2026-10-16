// ABOUTME: A check plugin run as a `Check` (ADR-089): its request and fact files, its plan, the findings and
// ABOUTME: result it reports, every protocol violation refused as an error, and SIGTERM before SIGKILL.
#![cfg(unix)]

mod support;

use std::path::Path;
use std::time::{Duration, Instant};

use cupel_core::{
    Cancellation, Check, CheckCtx, CheckRegistry, CheckStatus, CostClass, CoveredLines, Error,
    Event, Facts, PerTestMap, Plan, ProducerId, RunId, Scope, TestCoverage, Writer,
};
use serde_json::{Map, Value, json};
use support::{check_manifest, discovery, entries, languages, plugin, prints, script};

/// A check plugin in `dir` answering `plan` with `plan` and `run` with `run`, registered.
fn registered(dir: &Path, plan: &str, run: &str) -> CheckRegistry {
    let program = script(dir, "check.sh", &plugin(dir, &check_manifest(), plan, run));
    let entries = entries(&[("sample", &program)]);
    let languages = languages();
    let cancel = Cancellation::new();
    let plugins = cupel_plugin::discover(&discovery(dir, &entries, &languages, &cancel)).unwrap();
    let mut checks = CheckRegistry::default();
    plugins.register_checks(&mut checks);
    checks
}

fn ctx(root: &Path, cancel: Cancellation) -> CheckCtx {
    CheckCtx {
        scope: Scope::of_files(vec!["src/a.ts".to_owned(), "src/b.ts".to_owned()]),
        config: Map::from_iter([("max".to_owned(), json!(3))]),
        writer: Writer::new(root, false)
            .unwrap()
            .scratch(&RunId::generate()),
        cancel,
    }
}

fn map() -> PerTestMap {
    PerTestMap::new(
        ProducerId::from("tool.vitest"),
        vec![TestCoverage::new(
            "adds",
            None,
            vec![CoveredLines {
                file: "src/a.ts".to_owned(),
                lines: vec![1],
            }],
        )],
    )
}

fn facts(check: &dyn Check) -> Facts {
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(map()).unwrap();
    facts
}

fn finding(kind: &str, evidence: &Value) -> Value {
    json!({
        "ev": "finding", "id": "fnd_0a1b2c", "check": "ref.sample", "kind": kind,
        "severity": "warn", "confidence": 1.0, "tier": 2, "symbol": null,
        "location": {"file": "src/b.ts", "line": 1, "col": null, "end_line": 1, "end_col": null},
        "title": "src/b.ts is untested", "detail": null, "evidence": evidence,
        "action": {"kind": "write_test", "priority": 1, "summary": "Write a test.", "hints": {}},
        "repro": "cupel check --only ref.sample", "suppress": "cupel suppress fnd_0a1b2c",
        "new_since_baseline": false, "suppressed": false
    })
}

fn untested() -> Value {
    finding("ref.sample-file", &json!({"file": "src/b.ts"}))
}

fn result(status: &str) -> Value {
    json!({
        "ev": "result", "status": status,
        "score": {"value": 0.5, "unit": "tested_files", "n": 2},
        "metrics": {"untested": 1}, "notes": []
    })
}

fn progress() -> Value {
    json!({"ev": "progress", "id": "ref.sample", "done": 2, "total": 2, "eta_ms": null})
}

/// Runs the registered plugin in `dir` and returns its outcome and the events it emitted.
fn run(dir: &Path, script_run: &str) -> (cupel_core::Result<cupel_core::CheckResult>, Vec<Event>) {
    let checks = registered(dir, "exit 1", script_run);
    let check = checks.iter().next().unwrap();
    let ctx = ctx(dir, Cancellation::new());
    let mut events = Vec::new();
    let plan = Plan::estimate(CostClass::Quick, 2);
    let ran = check.run(&ctx, &facts(check), &plan, &mut events);
    (ran, events)
}

/// A finding by its kind, any other event by its `ev` tag.
fn event_name(event: &Event) -> String {
    let json = serde_json::to_value(event).unwrap();
    json.get("kind")
        .unwrap_or(&json["ev"])
        .as_str()
        .unwrap()
        .to_owned()
}

fn plugin_problem(ran: cupel_core::Result<cupel_core::CheckResult>) -> String {
    match ran {
        Err(Error::Plugin { plugin, problem }) => {
            assert_eq!(plugin, "sample");
            problem
        }
        other => panic!("expected a plugin error, got {other:?}"),
    }
}

#[test]
fn a_check_plugin_reports_its_findings_and_result_through_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let (ran, events) = run(
        dir.path(),
        &prints(&[untested(), progress(), result("fail")]),
    );
    let ran = ran.unwrap();
    assert_eq!(
        (ran.id.as_str(), ran.status, ran.findings, ran.tier.value()),
        ("ref.sample", CheckStatus::Fail, 1, 2)
    );
    assert_eq!(ran.metrics.get("untested").copied(), Some(1.0));
    assert_eq!(ran.score.unwrap().unit, "tested_files");
    assert!(ran.notes.is_empty(), "{:?}", ran.notes);
    assert_eq!(
        events.iter().map(event_name).collect::<Vec<_>>(),
        ["ref.sample-file", "progress"]
    );
}

#[test]
fn the_request_carries_the_scope_config_and_a_file_per_declared_fact() {
    let dir = tempfile::tempdir().unwrap();
    let (ran, _) = run(dir.path(), &prints(&[result("pass")]));
    assert_eq!(ran.unwrap().status, CheckStatus::Pass);
    let request: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("request.json")).unwrap())
            .unwrap();
    assert_eq!(request["protocol"], 1);
    assert_eq!(
        request["scope"],
        json!({"files": ["src/a.ts", "src/b.ts"], "symbols": null})
    );
    assert_eq!(request["config"], json!({"max": 3}));
    assert_eq!(
        (&request["seed"], &request["depth"]),
        (&Value::Null, &Value::Null)
    );
    assert_eq!(request["write"], false);
    let facts = request["facts"].as_object().unwrap();
    // `symbols` is wanted but was not produced, so it has no file.
    assert_eq!(facts.keys().collect::<Vec<_>>(), ["per-test-map"]);
    let file = Path::new(facts["per-test-map"].as_str().unwrap());
    let written: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    assert_eq!(written, serde_json::to_value(map()).unwrap());
    let scratch = Path::new(request["scratch"].as_str().unwrap());
    assert!(
        scratch.ends_with("plugins/ref.sample") && scratch.is_dir(),
        "{scratch:?}"
    );
    assert!(file.starts_with(scratch));
    assert!(scratch.starts_with(dir.path().canonicalize().unwrap().join(".cupel/work")));
    assert_eq!(request["budget_ms"], Value::Null);
}

#[test]
fn plan_asks_the_plugin_and_falls_back_to_the_cost_estimate_when_it_fails() {
    let dir = tempfile::tempdir().unwrap();
    let scope = Scope::of_files(vec!["src/a.ts".to_owned(), "src/b.ts".to_owned()]);
    let planned = |plan: &str| {
        let checks = registered(dir.path(), plan, "exit 1");
        let check = checks.iter().next().unwrap();
        check
            .plan(&ctx(dir.path(), Cancellation::new()), &scope)
            .unwrap()
    };
    assert_eq!(
        planned(r#"echo '{"targets": 7, "est_ms": 40}'"#),
        Plan {
            targets: 7,
            est_ms: 40
        }
    );
    assert_eq!(planned("exit 2"), Plan::estimate(CostClass::Quick, 2));
    assert_eq!(
        planned("echo nonsense"),
        Plan::estimate(CostClass::Quick, 2)
    );
}

#[test]
fn findings_the_manifest_does_not_declare_or_describe_are_refused_and_nothing_is_emitted() {
    let mut other_check = untested();
    other_check["check"] = json!("ref.other");
    let mut bad_progress = progress();
    bad_progress["id"] = json!("ref.other");
    let cases = [
        (
            finding("ref.other-file", &json!({"file": "src/b.ts"})),
            "finding kind `ref.other-file` is not declared in the manifest",
        ),
        (
            other_check,
            "a finding of check `ref.other`, not `ref.sample`",
        ),
        (
            finding("ref.sample-file", &json!({"file": 3})),
            "`ref.sample-file` evidence: evidence.file: 3 is not of type \"string\"",
        ),
        (
            bad_progress,
            "progress of check `ref.other`, not `ref.sample`",
        ),
    ];
    let (events, expected): (Vec<Value>, Vec<&str>) = cases.into_iter().unzip();
    let outcomes: Vec<(String, usize)> = events
        .into_iter()
        .map(|event| {
            let dir = tempfile::tempdir().unwrap();
            let (ran, emitted) = run(dir.path(), &prints(&[event, result("fail")]));
            (plugin_problem(ran), emitted.len())
        })
        .collect();
    let expected: Vec<(String, usize)> = expected
        .iter()
        .map(|problem| ((*problem).to_owned(), 0))
        .collect();
    assert_eq!(outcomes, expected);
}

#[test]
fn results_outside_the_protocol_are_refused() {
    let mut undeclared_metric = result("pass");
    undeclared_metric["metrics"] = json!({"lines": 3});
    let fact = json!({"ev": "fact", "kind": "per-test-map", "version": 2, "payload": {}});
    let cases: [(Vec<Value>, &str); 5] = [
        (
            vec![undeclared_metric],
            "metric `lines` is not a declared scope metric",
        ),
        (
            vec![result("skipped")],
            "a result's status must be pass, fail, partial, or error",
        ),
        (vec![result("pass"), result("pass")], "more than one result"),
        (vec![fact, result("pass")], "a check plugin printed a fact"),
        (vec![], "no result"),
    ];
    let (outputs, expected): (Vec<Vec<Value>>, Vec<&str>) = cases.into_iter().unzip();
    let mut problems: Vec<String> = outputs
        .iter()
        .map(|lines| {
            let dir = tempfile::tempdir().unwrap();
            plugin_problem(run(dir.path(), &prints(lines)).0)
        })
        .collect();
    let dir = tempfile::tempdir().unwrap();
    problems.push(plugin_problem(
        run(dir.path(), "echo '{\"ev\": \"plan\"}'").0,
    ));
    let mut expected: Vec<String> = expected
        .iter()
        .map(|problem| (*problem).to_owned())
        .collect();
    expected.push("line 1 is not a protocol event: unknown variant `plan`, expected one of `finding`, `progress`, `result`, `fact` at line 1 column 13".to_owned());
    assert_eq!(problems, expected);
}

#[test]
fn a_non_zero_exit_without_a_result_is_an_error_and_with_one_is_noted() {
    let dir = tempfile::tempdir().unwrap();
    let problem = plugin_problem(run(dir.path(), "echo 'out of memory' >&2; exit 2").0);
    assert_eq!(
        problem,
        "exited with code 2 without a result: out of memory"
    );
    let (ran, _) = run(
        dir.path(),
        &format!("{}\nexit 1", prints(&[result("pass")])),
    );
    let ran = ran.unwrap();
    assert_eq!(ran.status, CheckStatus::Pass);
    assert_eq!(
        ran.notes,
        ["plugin `sample` exited with code 1 after its result"]
    );
}

#[test]
fn a_cancelled_run_sends_sigterm_then_sigkill_and_is_cut_short() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("terminated");
    let body = format!(
        "trap 'echo term > {}; exit 0' TERM; while true; do sleep 0.05; done",
        marker.display()
    );
    let checks = registered(dir.path(), "exit 1", &body);
    let check = checks.iter().next().unwrap();
    let started = Instant::now();
    let cancel = Cancellation::with_deadline(started + Duration::from_millis(400));
    let ran = check.run(
        &ctx(dir.path(), cancel),
        &facts(check),
        &Plan::estimate(CostClass::Quick, 2),
        &mut Vec::new(),
    );
    assert_eq!(ran, Err(Error::Cancelled));
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "term\n");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    let request: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("request.json")).unwrap())
            .unwrap();
    let budget = request["budget_ms"].as_u64().unwrap();
    assert!(budget > 0 && budget <= 400, "{budget}");
}
