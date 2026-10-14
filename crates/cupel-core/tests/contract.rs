// ABOUTME: Tests that the output contract types reproduce design §7 exactly: envelope, finding, events.
// ABOUTME: Design examples must deserialize and re-serialize with every key and value intact.
use cupel_core::{
    ActionKind, CheckStatus, ConfigSource, CostClass, Envelope, Event, ExitCode, Finding,
    ScopeMode, Severity, SkipReason, Status, Tier, Verb,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// Asserts every key and value in `expected` appears identically in `actual`. Arrays must
/// match element for element. `actual` may carry extra keys only where they are `null`
/// (ADR-013: absent optional values are emitted as `null`).
fn assert_contains(expected: &Value, actual: &Value, path: &str) {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            for (key, e_value) in e {
                let a_value = a
                    .get(key)
                    .unwrap_or_else(|| panic!("{path}.{key} missing from output"));
                assert_contains(e_value, a_value, &format!("{path}.{key}"));
            }
            for (key, a_value) in a {
                assert!(
                    e.contains_key(key) || a_value.is_null(),
                    "{path}.{key} = {a_value} is not in the design example"
                );
            }
        }
        (Value::Array(e), Value::Array(a)) => {
            assert_eq!(e.len(), a.len(), "{path} length");
            for (i, (e_item, a_item)) in e.iter().zip(a).enumerate() {
                assert_contains(e_item, a_item, &format!("{path}[{i}]"));
            }
        }
        _ => assert_eq!(expected, actual, "{path}"),
    }
}

/// Deserializes `example` as `T`, re-serializes it, and checks the result against the example.
fn assert_round_trip<T: Serialize + DeserializeOwned>(example: &Value) -> Value {
    let typed: T = serde_json::from_value(example.clone()).expect("example deserializes");
    let output = serde_json::to_value(&typed).expect("value serializes");
    assert_contains(example, &output, "$");
    let again: T = serde_json::from_value(output.clone()).expect("output deserializes");
    assert_eq!(serde_json::to_value(&again).unwrap(), output);
    output
}

/// The finding from design §7.2, with the elided hashes filled in.
fn design_finding() -> Value {
    json!({
        "id": "fnd_7c2e1a",
        "check": "tests.mutation", "kind": "mutant.survived",
        "severity": "fail", "confidence": 0.95, "tier": 3,
        "symbol": {"display": "exonym::rewrite::Planner::push_limit", "hash": "sh_4b1d9e0c4b1d9e0c", "lang": "rust"},
        "location": {"file": "src/rewrite/planner.rs", "line": 142, "col": 21, "end_line": 142, "end_col": 23},
        "title": "Mutant survived: `<=` → `<` at planner.rs:142",
        "detail": "No test distinguishes limit == offset from limit < offset.",
        "evidence": {"mutation": {"from": "<=", "to": "<", "operator": "relational"},
                     "tests_run": 118, "covering_tests": ["rewrite::limit_tests::pushes_through_sort"]},
        "action": {"kind": "write_test", "priority": 1,
                   "summary": "Add a test where limit == offset asserting the boundary.",
                   "hints": {"nearest_test_file": "tests/rewrite/limit_tests.rs",
                             "covering_tests": ["pushes_through_sort"]}},
        "repro": "cupel check --only tests.mutation --symbols sh_4b1d9e0c4b1d9e0c --seed 42",
        "suppress": "cupel suppress fnd_7c2e1a --reason '…'",
        "new_since_baseline": true, "suppressed": false
    })
}

/// The envelope from design §7.1, with elided IDs and the finding filled in, and the check's
/// `metrics` (ADR-060).
fn design_envelope() -> Value {
    json!({
        "cupel": {"version": "0.4.0", "schema": "https://cupel.dev/schema/v1/run.json"},
        "run": {
            "id": "run_01JA2B3C4D5E6F7G8H9JKMNPQR", "verb": "check",
            "started": "2026-10-10T16:31:04Z", "duration_ms": 48211,
            "git": {"sha": "9f1c2d3e", "dirty": false, "base": "a81e4f5a", "branch": "feat/limit-pushdown"},
            "scope": {"mode": "changed", "files": 4, "symbols": 11, "roots": ["."], "languages": ["rust"]},
            "depth": "standard", "budget_ms": null, "seed": 42,
            "toolchain": {"rust": "1.93.0", "cargo-llvm-cov": "0.6.12", "cargo-nextest": "0.9.80"},
            "config": {"effective": {"depth": "standard"}, "sources": ["flags", "cupel.toml"]},
            "baseline": {"id": "run_01J9ZZZZZZZZZZZZZZZZZZZZZZ", "name": "default"}
        },
        "status": "fail",
        "exit_code": 1,
        "checks": [
            {"id": "tests.mutation", "status": "fail", "tier": 3, "cost": "standard",
             "score": {"value": 0.71, "unit": "killed_ratio", "n": 34},
             "metrics": {"score": 0.71},
             "delta": {"vs": "baseline", "value": -0.12},
             "findings": 6, "duration_ms": 41000, "skipped": null, "oracle": null}
        ],
        "findings": [design_finding()],
        "summary": {
            "findings": {"fail": 6, "warn": 9, "info": 22},
            "new": 4, "resolved": 2, "suppressed": 1,
            "gate": {"result": "fail", "evaluated": "gate.changed",
                     "rules": [{"rule": "tests.mutation.score >= 0.80", "actual": 0.71, "result": "fail"}]}
        },
        "next_actions": [
            {"priority": 1, "finding": "fnd_7c2e1a", "kind": "write_test", "summary": "Add a boundary test."},
            {"priority": 2, "kind": "rerun", "summary": "Run differential vs oracle 'pg'",
             "command": "cupel check --changed --depth deep --only gen.differential"}
        ],
        "truncated": false, "total_findings": 37
    })
}

#[test]
fn design_envelope_round_trips_exactly() {
    let output = assert_round_trip::<Envelope>(&design_envelope());
    // ADR-013: absent optional values are emitted as null, never omitted.
    assert_eq!(output["next_actions"][0]["command"], Value::Null);
    assert_eq!(output["next_actions"][1]["finding"], Value::Null);
}

#[test]
fn design_finding_round_trips_exactly() {
    assert_round_trip::<Finding>(&design_finding());
}

#[test]
fn envelope_for_a_verb_without_scope_or_git_uses_nulls() {
    let mut example = design_envelope();
    let run = example["run"].as_object_mut().unwrap();
    run.insert("verb".into(), json!("describe"));
    for key in ["git", "scope", "depth", "baseline"] {
        run.insert(key.into(), Value::Null);
    }
    let output = assert_round_trip::<Envelope>(&example);
    assert_eq!(output["run"]["git"], Value::Null);
}

#[test]
fn skipped_check_result_carries_reason_fact_lang_and_pointer() {
    let mut example = design_envelope();
    example["checks"][0] = json!({
        "id": "static.complexity", "status": "skipped", "tier": 0, "cost": "quick",
        "score": null, "metrics": {}, "delta": null, "findings": 0, "duration_ms": 0,
        "skipped": {"reason": "unsupported", "fact": "decisions", "lang": "go", "tool": null,
                    "see": "cupel describe matrix"},
        "oracle": null
    });
    assert_round_trip::<Envelope>(&example);
}

#[test]
fn finding_without_symbol_or_location_round_trips() {
    let mut example = design_finding();
    example["symbol"] = Value::Null;
    example["location"] = Value::Null;
    example["detail"] = Value::Null;
    assert_round_trip::<Finding>(&example);
}

#[test]
fn location_columns_are_optional() {
    let mut example = design_finding();
    example["location"] =
        json!({"file": "go.mod", "line": 1, "col": null, "end_line": 3, "end_col": null});
    assert_round_trip::<Finding>(&example);
}

#[test]
fn design_event_lines_round_trip_exactly() {
    let mut finding_event = design_finding();
    finding_event
        .as_object_mut()
        .unwrap()
        .insert("ev".into(), json!("finding"));
    for line in [
        json!({"ev": "plan", "run": "run_01JA2B3C4D5E6F7G8H9JKMNPQR", "checks": 7, "est_ms": 180_000}),
        json!({"ev": "check.start", "id": "tests.mutation", "targets": 34}),
        finding_event,
        json!({"ev": "progress", "id": "tests.mutation", "done": 12, "total": 34, "eta_ms": 61000}),
        json!({"ev": "check.end", "id": "tests.mutation", "status": "fail", "duration_ms": 41000}),
        json!({"ev": "skip", "id": "gen.fuzz", "reason": "budget"}),
        json!({"ev": "run.end", "status": "fail", "exit_code": 1}),
    ] {
        assert_round_trip::<Event>(&line);
    }
}

/// The design §6.8 plan as ADR-020 types it: skips carry the `Skipped` fields beside the ID,
/// and writes are objects with an optional diff.
fn design_plan() -> Value {
    json!({
        "would_run": [
            {"id": "static.crap", "targets": 11, "est_ms": 900, "needs": ["coverage(cached)"], "note": null},
            {"id": "tests.mutation", "targets": 34, "est_ms": 140_000,
             "needs": ["coverage(fresh)", "per-test-map"],
             "note": "19/34 sites cached as killed; 15 will execute; schemata build x1"}
        ],
        "would_skip": [
            {"id": "conc.dst", "reason": "unconfigured", "fact": null, "lang": null, "tool": null,
             "see": "cupel describe conc.dst"}
        ],
        "would_install": [{"tool": "cargo-llvm-cov", "command": "cargo install cargo-llvm-cov"}],
        "would_write": [
            {"path": ".cupel/runs/run_01JA2B3C4D5E6F7G8H9JKMNPQR.json", "diff": null},
            {"path": "cupel.toml", "diff": "--- /dev/null\n+++ b/cupel.toml\n@@ -0,0 +1 @@\n+depth = \"quick\"\n"}
        ],
        "est_total_ms": 140_900
    })
}

#[test]
fn design_plan_round_trips_exactly_inside_a_plan_envelope() {
    let mut example = design_envelope();
    example["status"] = json!("plan");
    example["exit_code"] = json!(0);
    example["plan"] = design_plan();
    let output = assert_round_trip::<Envelope>(&example);
    assert_eq!(output["plan"], design_plan());
}

#[test]
fn envelopes_without_a_plan_carry_a_null_plan() {
    let output = assert_round_trip::<Envelope>(&design_envelope());
    assert_eq!(output["plan"], Value::Null);
}

#[test]
fn envelopes_without_an_explanation_carry_a_null_explanation() {
    let output = assert_round_trip::<Envelope>(&design_envelope());
    assert_eq!(output["explanation"], Value::Null);
}

#[test]
fn an_explanation_round_trips_inside_an_envelope() {
    let mut example = design_envelope();
    example["explanation"] = json!({
        "subject": "finding", "target": "fnd_7c2e1a", "run": "run_01JA2B3C4D5E6F7G8H9JKMNPQR",
        "excerpt": {"file": "src/rewrite/planner.rs", "lines": [
            {"line": 141, "text": "fn push_limit() {", "focus": false},
            {"line": 142, "text": "    if limit <= offset {", "focus": true}
        ]},
        "covering_tests": ["pushes_through_sort"],
        "nearest_test_file": "tests/rewrite/limit_tests.rs",
        "metrics": [{"symbol": "sh_4b1d9e0c4b1d9e0c", "key": "cyclomatic", "value": 7.0},
                    {"symbol": null, "key": "score", "value": 0.5}]
    });
    let output = assert_round_trip::<Envelope>(&example);
    assert_eq!(output["explanation"], example["explanation"]);
}

#[test]
fn unknown_event_tag_is_rejected() {
    assert!(serde_json::from_value::<Event>(json!({"ev": "run.start"})).is_err());
}

#[test]
fn exit_codes_have_the_design_numbers_and_names() {
    let table = [
        (ExitCode::Pass, 0, "pass"),
        (ExitCode::Fail, 1, "fail"),
        (ExitCode::Usage, 2, "usage"),
        (ExitCode::Environment, 3, "environment"),
        (ExitCode::Unevaluable, 4, "unevaluable"),
        (ExitCode::Partial, 5, "partial"),
        (ExitCode::Internal, 6, "internal"),
    ];
    assert_eq!(ExitCode::ALL.len(), table.len());
    for (code, number, name) in table {
        assert_eq!(code.code(), number);
        assert_eq!(code.name(), name);
        assert_eq!(serde_json::to_value(code).unwrap(), json!(number));
        assert_eq!(
            serde_json::from_value::<ExitCode>(json!(number)).unwrap(),
            code
        );
        assert_eq!(ExitCode::try_from(number).unwrap(), code);
    }
}

#[test]
fn exit_code_outside_0_to_6_is_rejected() {
    assert!(serde_json::from_value::<ExitCode>(json!(7)).is_err());
    assert!(serde_json::from_value::<ExitCode>(json!("pass")).is_err());
}

#[test]
fn tier_accepts_0_to_6_only() {
    for n in 0..=6u8 {
        let tier = Tier::try_from(n).unwrap();
        assert_eq!(tier.value(), n);
        assert_eq!(serde_json::to_value(tier).unwrap(), json!(n));
    }
    assert!(Tier::try_from(7).is_err());
    assert!(serde_json::from_value::<Tier>(json!(7)).is_err());
}

/// Asserts each value serializes to the given JSON string and back.
fn assert_spellings<T>(cases: &[(T, &str)])
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    for (value, text) in cases {
        assert_eq!(serde_json::to_value(value).unwrap(), json!(text));
        assert_eq!(&serde_json::from_value::<T>(json!(text)).unwrap(), value);
    }
}

#[test]
fn status_values_match_design() {
    assert_spellings(&[
        (Status::Pass, "pass"),
        (Status::Fail, "fail"),
        (Status::Partial, "partial"),
        (Status::Error, "error"),
        (Status::Plan, "plan"),
    ]);
}

#[test]
fn check_status_values() {
    assert_spellings(&[
        (CheckStatus::Pass, "pass"),
        (CheckStatus::Fail, "fail"),
        (CheckStatus::Partial, "partial"),
        (CheckStatus::Error, "error"),
        (CheckStatus::Skipped, "skipped"),
    ]);
}

#[test]
fn severity_values_match_design() {
    assert_spellings(&[
        (Severity::Fail, "fail"),
        (Severity::Warn, "warn"),
        (Severity::Info, "info"),
    ]);
}

#[test]
fn action_kinds_are_the_closed_design_set() {
    let cases = [
        (ActionKind::WriteTest, "write_test"),
        (ActionKind::StrengthenAssertion, "strengthen_assertion"),
        (ActionKind::RestoreTest, "restore_test"),
        (ActionKind::RefactorExtract, "refactor_extract"),
        (ActionKind::ReduceComplexity, "reduce_complexity"),
        (ActionKind::BreakCycle, "break_cycle"),
        (ActionKind::FixBug, "fix_bug"),
        (ActionKind::AddContract, "add_contract"),
        (ActionKind::AddProperty, "add_property"),
        (ActionKind::InvestigateFlake, "investigate_flake"),
        (ActionKind::Rerun, "rerun"),
        (ActionKind::Configure, "configure"),
        (ActionKind::InstallTool, "install_tool"),
        (ActionKind::VerifyOracle, "verify_oracle"),
        (ActionKind::Review, "review"),
    ];
    assert_eq!(ActionKind::ALL.len(), cases.len());
    assert_spellings(&cases);
    assert!(serde_json::from_value::<ActionKind>(json!("delete_test")).is_err());
}

#[test]
fn cost_classes_match_depth_names() {
    assert_spellings(&[
        (CostClass::Quick, "quick"),
        (CostClass::Standard, "standard"),
        (CostClass::Deep, "deep"),
        (CostClass::Exhaustive, "exhaustive"),
    ]);
}

#[test]
fn verbs_are_the_design_set() {
    let cases = [
        (Verb::Init, "init"),
        (Verb::Doctor, "doctor"),
        (Verb::Describe, "describe"),
        (Verb::Check, "check"),
        (Verb::Survey, "survey"),
        (Verb::Explain, "explain"),
        (Verb::Baseline, "baseline"),
        (Verb::Oracle, "oracle"),
        (Verb::Tests, "tests"),
        (Verb::Suppress, "suppress"),
        (Verb::Serve, "serve"),
    ];
    assert_eq!(Verb::ALL.len(), cases.len());
    assert_spellings(&cases);
}

#[test]
fn scope_modes() {
    assert_spellings(&[
        (ScopeMode::Tree, "tree"),
        (ScopeMode::Changed, "changed"),
        (ScopeMode::Range, "range"),
        (ScopeMode::Paths, "paths"),
        (ScopeMode::Symbols, "symbols"),
        (ScopeMode::SinceBaseline, "since-baseline"),
    ]);
}

#[test]
fn config_sources() {
    assert_spellings(&[
        (ConfigSource::Flags, "flags"),
        (ConfigSource::Env, "env"),
        (ConfigSource::Profile, "profile"),
        (ConfigSource::CupelToml, "cupel.toml"),
        (ConfigSource::Defaults, "defaults"),
    ]);
}

#[test]
fn skip_reasons() {
    assert_spellings(&[
        (SkipReason::Unsupported, "unsupported"),
        (SkipReason::Unavailable, "unavailable"),
        (SkipReason::Unconfigured, "unconfigured"),
        (SkipReason::Budget, "budget"),
        (SkipReason::Timeout, "timeout"),
        (SkipReason::NeedsRange, "needs-range"),
    ]);
}
