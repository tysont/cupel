// ABOUTME: Redaction tests: every volatile field of ADR-003 §2 is masked, nulls stay null, and
// ABOUTME: everything that is part of the contract survives unchanged.
use std::path::Path;

use cupel_core::Envelope;
use cupel_fixtures::{redact, redact_sarif};
use serde_json::{Value, json};

/// An envelope with every volatile field set to a value that appears nowhere else.
fn volatile_envelope(root: &str, canonical_root: &str) -> Envelope {
    let value = json!({
        "cupel": {"version": "9.8.7-volatile", "schema": "https://tysont.github.io/cupel/schema/v1/run.json"},
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
            "metrics": {"build_ms": 654_321.0, "score": 0.5},
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
            "evidence": {"timing": {"duration_ms": 765_432}, "actual": "9.8.7-volatile"},
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
            "config_precedence": [], "install_routes": []
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
        "654321",
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
        ("/checks/0/metrics/build_ms", "<duration-ms>"),
        ("/findings/0/evidence/timing/duration_ms", "<duration-ms>"),
        ("/findings/0/evidence/actual", "<version>"),
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
            json!("https://tysont.github.io/cupel/schema/v1/run.json"),
        ),
        ("/run/verb", json!("check")),
        ("/run/budget_ms", json!(null)),
        ("/run/config/sources", json!(["flags"])),
        ("/run/baseline/name", json!("default")),
        ("/status", json!("fail")),
        ("/exit_code", json!(1)),
        ("/checks/0/metrics/score", json!(0.5)),
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
        &json!("https://tysont.github.io/cupel/schema/v1/run.json")
    );
    assert_eq!(
        pointer(&redacted, "/findings/0/location/file"),
        &json!("/nowhere/src/lib.rs")
    );
}

#[test]
fn an_oracles_verification_time_and_run_are_redacted_and_an_unverified_ones_stay_null() {
    let mut value = serde_json::to_value(volatile_envelope("/nowhere", "/nowhere")).unwrap();
    let definition =
        json!({"kind": "ref", "ref": "v1", "cmd": null, "spec": null, "lang": null, "inputs": []});
    value["oracles"] = json!([
        {"name": "release", "definition": definition, "state": "verified", "hash": "ab12",
         "verified_at": "1999-12-31T23:59:59Z", "verified_run": "run_01ARZ3NDEKTSV4RRFFQ69G5FAV"},
        {"name": "next", "definition": definition, "state": "unverified", "hash": "cd34",
         "verified_at": null, "verified_run": null},
    ]);
    let envelope: Envelope = serde_json::from_value(value).unwrap();

    let redacted = redact(&envelope, Path::new("/nowhere"));

    assert_eq!(
        (
            pointer(&redacted, "/oracles/0/verified_at"),
            pointer(&redacted, "/oracles/0/verified_run"),
            pointer(&redacted, "/oracles/0/hash"),
        ),
        (&json!("<timestamp>"), &json!("<run-id>"), &json!("ab12"))
    );
    assert_eq!(pointer(&redacted, "/oracles/1/verified_run"), &Value::Null);
}

/// A SARIF log shaped as `--format sarif` renders `volatile_envelope`: the run in the
/// invocation's properties, the rest of the envelope in the run's, the root as file URIs.
fn volatile_sarif(root: &str, canonical_root: &str) -> Value {
    let envelope = serde_json::to_value(volatile_envelope(root, canonical_root)).unwrap();
    let mut rest = envelope.as_object().unwrap().clone();
    rest.remove("run");
    rest.remove("findings");
    let uri = format!("file://{canonical_root}/");
    json!({
        "version": "2.1.0",
        "runs": [{
            "tool": {"driver": {"name": "cupel", "version": "9.8.7-volatile", "semanticVersion": "9.8.7-volatile"}},
            "automationDetails": {"id": "cupel/check/run_01ARZ3NDEKTSV4RRFFQ69G5FAV"},
            "invocations": [{
                "executionSuccessful": true,
                "exitCode": 1,
                "startTimeUtc": "1999-12-31T23:59:59Z",
                "properties": envelope["run"],
            }],
            "originalUriBaseIds": {"SRCROOT": {"uri": uri}},
            "versionControlProvenance": [{"repositoryUri": uri, "revisionId": "deadbeefcafe", "branch": "volatile-branch"}],
            "results": [{
                "ruleId": "complexity.high",
                "message": {"text": format!("Read {root}/src/lib.rs")},
                "properties": {"evidence": envelope["findings"][0]["evidence"]},
            }],
            "properties": rest,
        }],
    })
}

#[test]
fn a_sarif_log_is_redacted_where_its_envelope_would_be() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap().to_owned();
    let canonical = dir.path().canonicalize().unwrap();
    let canonical = canonical.to_str().unwrap().to_owned();

    let redacted = redact_sarif(&volatile_sarif(&root, &canonical), dir.path());

    let text = redacted.to_string();
    for volatile in [
        "9.8.7-volatile",
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "01BX5ZZKBKACTAV9WEVGEMMVRZ",
        "1999-12-31",
        "987654",
        "876543",
        "765432",
        "654321",
        "424242",
        "1.111.0-volatile",
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
    let run = "/runs/0";
    let invocation = "/runs/0/invocations/0";
    for (path, placeholder) in [
        (format!("{run}/tool/driver/version"), "<version>"),
        (format!("{run}/tool/driver/semanticVersion"), "<version>"),
        (
            format!("{run}/automationDetails/id"),
            "cupel/check/<run-id>",
        ),
        (format!("{invocation}/startTimeUtc"), "<timestamp>"),
        (format!("{invocation}/properties/id"), "<run-id>"),
        (format!("{invocation}/properties/git/sha"), "<sha>"),
        (
            format!("{invocation}/properties/toolchain/rust"),
            "<version>",
        ),
        (
            format!("{run}/originalUriBaseIds/SRCROOT/uri"),
            "file://<root>/",
        ),
        (
            format!("{run}/versionControlProvenance/0/revisionId"),
            "<sha>",
        ),
        (
            format!("{run}/versionControlProvenance/0/branch"),
            "<branch>",
        ),
        (
            format!("{run}/results/0/message/text"),
            "Read <root>/src/lib.rs",
        ),
        (format!("{run}/properties/cupel/version"), "<version>"),
        (format!("{run}/properties/description/version"), "<version>"),
        (
            format!("{run}/properties/checks/0/duration_ms"),
            "<duration-ms>",
        ),
    ] {
        assert_eq!(pointer(&redacted, &path), &json!(placeholder), "at {path}");
    }
    assert_eq!(
        pointer(&redacted, "/runs/0/invocations/0/exitCode"),
        &json!(1)
    );
}
