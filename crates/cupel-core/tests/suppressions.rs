// ABOUTME: Tests for suppressions: the `.cupel/suppressions.toml` format, `--until` parsing, and
// ABOUTME: `apply_suppressions` marking, recounting, and reporting expiry (ADR-047).
use cupel_core::{
    CheckId, Envelope, FindingId, Severity, Suppression, Suppressions, Until, apply_suppressions,
};
use serde_json::{Value, json};

fn finding(id: &str, severity: &str) -> Value {
    json!({
        "id": id, "check": "doctor", "kind": "doctor.tool-missing",
        "severity": severity, "confidence": 1.0, "tier": 0,
        "symbol": null, "location": null, "title": format!("finding {id}"), "detail": null,
        "evidence": {}, "action": {"kind": "install_tool", "priority": 1, "summary": "Install.", "hints": {}},
        "repro": "cupel doctor", "suppress": format!("cupel suppress {id} --reason '…'"),
        "new_since_baseline": false, "suppressed": false
    })
}

fn envelope(findings: &[(&str, &str)]) -> Envelope {
    let findings: Vec<Value> = findings.iter().map(|(id, s)| finding(id, s)).collect();
    serde_json::from_value(json!({
        "cupel": {"version": "0.1.0", "schema": "https://cupel.dev/schema/v1/run.json"},
        "run": {
            "id": "run_01JA2B3C4D5E6F7G8H9JKMNPQA", "verb": "doctor",
            "started": "2026-10-13T10:00:00Z", "duration_ms": 1,
            "git": null, "scope": null, "depth": "standard", "budget_ms": null, "seed": 1,
            "toolchain": {}, "config": {"effective": {}, "sources": []}, "baseline": null
        },
        "status": "error", "exit_code": 3, "checks": [],
        "total_findings": findings.len(),
        "findings": findings,
        "summary": {"findings": {"fail": 1, "warn": 1, "info": 0}, "new": 0, "resolved": 0,
                    "suppressed": 0, "gate": null},
        "next_actions": [], "truncated": false
    }))
    .unwrap()
}

fn suppression(finding: &str, until: Option<&str>) -> Suppression {
    Suppression {
        finding: finding.parse().unwrap(),
        reason: "known".to_owned(),
        until: until.map(|until| until.parse().unwrap()),
        created: "2026-10-01".to_owned(),
    }
}

#[test]
fn until_is_a_date_when_shaped_like_one_and_a_ref_otherwise() {
    assert_eq!(
        "2026-12-31".parse::<Until>().unwrap(),
        Until::Date("2026-12-31".to_owned())
    );
    assert_eq!(
        "v1.2.0".parse::<Until>().unwrap(),
        Until::Ref("v1.2.0".to_owned())
    );
    assert_eq!(
        "origin/main".parse::<Until>().unwrap(),
        Until::Ref("origin/main".to_owned())
    );
    for bad in ["2026-13-01", "2026-02-32", "2026-00-10", "", "two words"] {
        assert!(bad.parse::<Until>().is_err(), "{bad}");
    }
}

#[test]
fn a_date_is_past_only_after_its_day() {
    let until = Until::Date("2026-10-13".to_owned());
    assert!(!until.date_passed("2026-10-12"));
    assert!(!until.date_passed("2026-10-13"));
    assert!(until.date_passed("2026-10-14"));
    assert!(!Until::Ref("v1".to_owned()).date_passed("2999-01-01"));
}

#[test]
fn the_file_round_trips_sorted_by_finding_with_a_header() {
    let mut file = Suppressions::default();
    file.upsert(suppression("fnd_00000b", Some("v2")));
    file.upsert(suppression("fnd_00000a", None));
    let text = file.to_toml();
    assert!(text.starts_with("# "), "{text}");
    assert_eq!(
        text.lines().skip(1).collect::<Vec<_>>().join("\n"),
        "[[suppression]]\nfinding = \"fnd_00000a\"\nreason = \"known\"\ncreated = \"2026-10-01\"\n\n\
         [[suppression]]\nfinding = \"fnd_00000b\"\nreason = \"known\"\nuntil = \"v2\"\ncreated = \"2026-10-01\""
    );
    assert_eq!(Suppressions::parse(&text).unwrap(), file);
}

#[test]
fn upsert_replaces_an_existing_entry() {
    let mut file = Suppressions::default();
    file.upsert(suppression("fnd_00000a", None));
    file.upsert(suppression("fnd_00000a", Some("2027-01-01")));
    assert_eq!(file.entries().len(), 1);
    assert_eq!(
        file.entries()[0].until,
        Some(Until::Date("2027-01-01".to_owned()))
    );
}

#[test]
fn malformed_files_are_rejected() {
    for bad in [
        "[[suppression]]\nfinding = \"nope\"\nreason = \"x\"\ncreated = \"2026-10-01\"\n",
        "[[suppression]]\nfinding = \"fnd_00000a\"\nreason = \"x\"\ncreated = \"2026-10-01\"\nextra = 1\n",
        "[[suppression]]\nfinding = \"fnd_00000a\"\nreason = \"x\"\ncreated = \"2026-10-01\"\nuntil = \"2026-99-99\"\n",
        "not toml at all [",
    ] {
        assert!(Suppressions::parse(bad).is_err(), "{bad}");
    }
    assert_eq!(Suppressions::parse("").unwrap(), Suppressions::default());
}

#[test]
fn active_suppressions_mark_findings_info_and_recount() {
    let mut envelope = envelope(&[("fnd_00000a", "fail"), ("fnd_00000b", "warn")]);
    apply_suppressions(&mut envelope, &[suppression("fnd_00000a", None)], |_| false);

    let a = &envelope.findings[0];
    assert!(a.suppressed);
    assert_eq!(a.severity, Severity::Info);
    assert!(!envelope.findings[1].suppressed);
    assert_eq!(envelope.summary.suppressed, 1);
    assert_eq!(envelope.summary.findings.fail, 0);
    assert_eq!(envelope.summary.findings.warn, 1);
    assert_eq!(envelope.summary.findings.info, 1);
}

#[test]
fn expired_suppressions_leave_the_finding_and_add_suppression_expired() {
    let mut envelope = envelope(&[("fnd_00000a", "fail"), ("fnd_00000b", "warn")]);
    apply_suppressions(
        &mut envelope,
        &[
            suppression("fnd_00000a", Some("2026-01-01")),
            suppression("fnd_0000ff", Some("2026-01-01")),
        ],
        |until| until.date_passed("2026-10-13"),
    );

    assert!(!envelope.findings[0].suppressed);
    assert_eq!(envelope.findings[0].severity, Severity::Fail);
    assert_eq!(envelope.findings.len(), 3, "absent findings get no expiry");
    let expired = &envelope.findings[2];
    assert_eq!(expired.kind, "suppression.expired");
    assert_eq!(expired.check, CheckId::from("suppress"));
    assert_eq!(expired.severity, Severity::Warn);
    assert_eq!(
        expired.id,
        FindingId::for_subject(
            &CheckId::from("suppress"),
            "suppression.expired",
            "fnd_00000a"
        )
    );
    assert_eq!(
        Value::Object(expired.evidence.0.clone()),
        json!({"finding": "fnd_00000a", "reason": "known", "until": "2026-01-01"})
    );
    assert_eq!(envelope.summary.suppressed, 0);
    assert_eq!(envelope.summary.findings.warn, 2);
    assert_eq!(envelope.summary.findings.fail, 1);
    assert_eq!(envelope.total_findings, 3);
}

#[test]
fn applying_twice_changes_nothing_more() {
    let mut once = envelope(&[("fnd_00000a", "fail")]);
    let suppressions = [suppression("fnd_00000a", None)];
    apply_suppressions(&mut once, &suppressions, |_| false);
    let mut twice = once.clone();
    apply_suppressions(&mut twice, &suppressions, |_| false);
    assert_eq!(once, twice);
}
