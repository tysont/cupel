// ABOUTME: Redaction tests: every volatile field of ADR-003 §2 is masked, nulls stay null, and
// ABOUTME: everything that is part of the contract survives unchanged.
use std::path::Path;

use cupel_core::Envelope;
use cupel_fixtures::redact;
use serde_json::{Value, json};

/// An envelope with every volatile field set to a value that appears nowhere else.
fn volatile_envelope(root: &str, canonical_root: &str) -> Envelope {
    let value = json!({
        "cupel": {"version": "9.8.7-volatile", "schema": "https://cupel.dev/schema/v1/run.json"},
        "run": {
            "id": "run_01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "verb": "check",
            "started": "1999-12-31T23:59:59Z",
            "duration_ms": 987_654,
            "git": {"sha": "deadbeefcafe", "dirty": true, "base": "feedface0123", "branch": "volatile-branch"},
            "scope": {"mode": "tree", "files": 3, "symbols": 7, "roots": [format!("{root}/pkg")], "languages": ["rust"]},
            "depth": "quick",
            "budget_ms": null,
            "seed": 424_242,
            "toolchain": {"rust": "1.111.0-volatile", "cargo-nextest": "0.9.999-volatile"},
            "config": {"effective": {"depth": "quick"}, "sources": ["flags"]},
            "baseline": {"id": "run_01BX5ZZKBKACTAV9WEVGEMMVRZ", "name": "default"}
        },
        "status": "fail",
        "exit_code": 1,
        "checks": [{
            "id": "static.complexity", "status": "fail", "tier": 1, "cost": "quick",
            "score": null, "delta": null, "findings": 1, "duration_ms": 876_543,
            "skipped": null, "oracle": null
        }],
        "findings": [{
            "id": "fnd_7c2e1a",
            "check": "static.complexity", "kind": "complexity.high",
            "severity": "fail", "confidence": 0.95, "tier": 1,
            "symbol": null,
            "location": {"file": format!("{canonical_root}/src/lib.rs"), "line": 3, "col": null, "end_line": 9, "end_col": null},
            "title": "Too complex",
            "detail": format!("Read {root}/src/lib.rs"),
            "evidence": {"timing": {"duration_ms": 765_432}},
            "action": {"kind": "reduce_complexity", "priority": 1, "summary": "Split it.", "hints": {}},
            "repro": "cupel check --seed 42",
            "suppress": "cupel suppress fnd_7c2e1a --reason '…'",
            "new_since_baseline": false,
            "suppressed": false
        }],
        "summary": {"findings": {"fail": 1, "warn": 0, "info": 0}, "new": 0, "resolved": 0, "suppressed": 0, "gate": null},
        "next_actions": [],
        "description": {
            "subject": "cli", "name": "cupel", "version": "9.8.7-volatile",
            "summary": "Cupel grades what a coding agent produced.", "stability": "experimental",
            "verbs": [], "global_flags": [], "flags": [], "exit_codes": [], "schemas": [],
            "config_precedence": []
        },
        "truncated": false,
        "total_findings": 1
    });
    serde_json::from_value(value).expect("the test envelope is well-typed")
}

fn pointer<'a>(value: &'a Value, path: &str) -> &'a Value {
    value
        .pointer(path)
        .unwrap_or_else(|| panic!("{path} is missing from {value:#}"))
}

#[test]
fn no_volatile_value_survives_redaction() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap().to_owned();
    let canonical = dir.path().canonicalize().unwrap();
    let canonical = canonical.to_str().unwrap().to_owned();
    let envelope = volatile_envelope(&root, &canonical);

    let text = redact(&envelope, dir.path()).to_string();

    for volatile in [
        "9.8.7-volatile",
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "01BX5ZZKBKACTAV9WEVGEMMVRZ",
        "1999-12-31",
        "987654",
        "876543",
        "765432",
        "424242",
        "1.111.0-volatile",
        "0.9.999-volatile",
        "deadbeefcafe",
        "feedface0123",
        "volatile-branch",
        root.as_str(),
        canonical.as_str(),
    ] {
        assert!(
            !text.contains(volatile),
            "{volatile:?} survived redaction:\n{text}"
        );
    }
}

#[test]
fn each_volatile_field_gets_its_placeholder() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap().to_owned();
    let canonical = dir.path().canonicalize().unwrap();
    let envelope = volatile_envelope(&root, canonical.to_str().unwrap());

    let redacted = redact(&envelope, dir.path());

    for (path, placeholder) in [
        ("/cupel/version", "<version>"),
        ("/description/version", "<version>"),
        ("/run/id", "<run-id>"),
        ("/run/baseline/id", "<run-id>"),
        ("/run/started", "<timestamp>"),
        ("/run/duration_ms", "<duration-ms>"),
        ("/checks/0/duration_ms", "<duration-ms>"),
        ("/findings/0/evidence/timing/duration_ms", "<duration-ms>"),
        ("/run/seed", "<seed>"),
        ("/run/toolchain/rust", "<version>"),
        ("/run/toolchain/cargo-nextest", "<version>"),
        ("/run/git/sha", "<sha>"),
        ("/run/git/base", "<sha>"),
        ("/run/git/branch", "<branch>"),
        ("/run/git/dirty", "<dirty>"),
        ("/run/scope/roots/0", "<root>/pkg"),
        ("/findings/0/location/file", "<root>/src/lib.rs"),
        ("/findings/0/detail", "Read <root>/src/lib.rs"),
    ] {
        assert_eq!(pointer(&redacted, path), &json!(placeholder), "at {path}");
    }
}

#[test]
fn contract_fields_survive_redaction() {
    let envelope = volatile_envelope("/nowhere", "/nowhere");

    let redacted = redact(&envelope, Path::new("/elsewhere"));

    for (path, expected) in [
        (
            "/cupel/schema",
            json!("https://cupel.dev/schema/v1/run.json"),
        ),
        ("/run/verb", json!("check")),
        ("/run/budget_ms", json!(null)),
        ("/run/config/sources", json!(["flags"])),
        ("/run/baseline/name", json!("default")),
        ("/status", json!("fail")),
        ("/exit_code", json!(1)),
        ("/findings/0/id", json!("fnd_7c2e1a")),
        ("/findings/0/location/file", json!("/nowhere/src/lib.rs")),
        ("/findings/0/repro", json!("cupel check --seed 42")),
        ("/description/name", json!("cupel")),
    ] {
        assert_eq!(pointer(&redacted, path), &expected, "at {path}");
    }
}

#[test]
fn absent_values_stay_null() {
    let mut envelope = volatile_envelope("/nowhere", "/nowhere");
    envelope.run.git = None;
    envelope.run.baseline = None;
    envelope.description = None;

    let redacted = redact(&envelope, Path::new("/nowhere"));

    assert_eq!(pointer(&redacted, "/run/git"), &Value::Null);
    assert_eq!(pointer(&redacted, "/run/baseline"), &Value::Null);
    assert_eq!(pointer(&redacted, "/description"), &Value::Null);
}

#[test]
fn the_cli_version_inside_describe_all_is_redacted() {
    let mut value = serde_json::to_value(volatile_envelope("/nowhere", "/nowhere")).unwrap();
    let cli = value["description"].take();
    value["description"] = json!({"subject": "all", "cli": cli, "commands": [], "schemas": []});
    value["description"]["cli"]
        .as_object_mut()
        .unwrap()
        .remove("subject");
    let envelope: Envelope = serde_json::from_value(value).unwrap();

    let redacted = redact(&envelope, Path::new("/nowhere"));

    assert_eq!(
        pointer(&redacted, "/description/cli/version"),
        &json!("<version>")
    );
}

#[test]
fn a_filesystem_root_is_never_treated_as_the_repository_root() {
    let envelope = volatile_envelope("/nowhere", "/nowhere");

    let redacted = redact(&envelope, Path::new("/"));

    assert_eq!(
        pointer(&redacted, "/cupel/schema"),
        &json!("https://cupel.dev/schema/v1/run.json")
    );
    assert_eq!(
        pointer(&redacted, "/findings/0/location/file"),
        &json!("/nowhere/src/lib.rs")
    );
}
