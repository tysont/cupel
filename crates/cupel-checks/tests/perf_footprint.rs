// ABOUTME: perf.footprint over synthetic Footprint facts: vulnerabilities, copyleft licenses, growth against the base,
// ABOUTME: which dependencies are reported with and without a base, notes for what was not measured, and the score (ADR-103).
mod support;

use cupel_core::{
    ActionKind, Artifact, CheckId, CheckStatus, CostClass, Dependency, FactKind, Finding,
    FindingId, Footprint, FootprintBase, MetricPer, Severity, Vulnerability,
};
use serde_json::json;
use support::{Shape, score};

const ID: &str = "perf.footprint";

fn dependency(name: &str, version: &str, license: Option<&str>, line: u32) -> Dependency {
    Dependency {
        name: name.to_owned(),
        version: Some(version.to_owned()),
        direct: true,
        license: license.map(str::to_owned),
        file: "Cargo.lock".to_owned(),
        line,
    }
}

fn vulnerability(package: &str, version: &str) -> Vulnerability {
    Vulnerability {
        id: "OSV-2099-0001".to_owned(),
        package: package.to_owned(),
        version: version.to_owned(),
        summary: "Out-of-bounds write".to_owned(),
        fixed: Some("0.6.10".to_owned()),
    }
}

/// A measured footprint: three dependencies, one copyleft, one vulnerable, and an artifact.
fn measured() -> Footprint {
    Footprint {
        manifests: vec!["Cargo.lock".to_owned()],
        dependencies: vec![
            dependency("serde", "1.0.0", Some("MIT OR Apache-2.0"), 4),
            dependency("gplthing", "0.1.0", Some("GPL-3.0-only"), 9),
            dependency("smallvec", "0.6.3", Some("MIT"), 14),
        ],
        licenses: true,
        vulnerabilities: Some(vec![vulnerability("smallvec", "0.6.3")]),
        artifacts: Some(vec![
            Artifact {
                path: "dist/a".to_owned(),
                bytes: 100,
            },
            Artifact {
                path: "dist/b".to_owned(),
                bytes: 24,
            },
        ]),
        base: None,
    }
}

fn kinds(findings: &[Finding]) -> Vec<(&str, &str)> {
    findings
        .iter()
        .map(|finding| {
            let package = finding.evidence.0.get("package").and_then(|p| p.as_str());
            (finding.kind.as_str(), package.unwrap_or("-"))
        })
        .collect()
}

#[test]
fn without_a_base_every_dependency_is_reported() {
    let (result, findings) = support::run_over(ID, measured());
    assert_eq!(
        kinds(&findings),
        [("dep.vuln", "smallvec"), ("dep.license", "gplthing")]
    );
    let vuln = &findings[0];
    assert_eq!(vuln.severity, Severity::Fail);
    assert_eq!(vuln.action.kind, ActionKind::FixBug);
    assert_eq!(
        vuln.evidence.0,
        *json!({"package": "smallvec", "version": "0.6.3", "advisory": "OSV-2099-0001",
                "summary": "Out-of-bounds write", "fixed": "0.6.10"})
        .as_object()
        .unwrap()
    );
    assert_eq!(
        vuln.id,
        FindingId::for_subject(
            &CheckId::from(ID),
            "dep.vuln",
            "smallvec@0.6.3|OSV-2099-0001"
        )
    );
    let at = vuln.location.as_ref().unwrap();
    assert_eq!((at.file.as_str(), at.line), ("Cargo.lock", 14));
    assert_eq!(vuln.repro, "cupel check --only perf.footprint");
    let license = &findings[1];
    assert_eq!(license.severity, Severity::Warn);
    assert_eq!(
        license.evidence.0,
        *json!({"package": "gplthing", "version": "0.1.0", "license": "GPL-3.0-only"})
            .as_object()
            .unwrap()
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(
        result.metrics,
        [
            ("artifact_bytes".to_owned(), 124.0),
            ("copyleft".to_owned(), 1.0),
            ("dependencies".to_owned(), 3.0),
            ("vulnerable".to_owned(), 1.0),
        ]
        .into()
    );
    assert_eq!(result.score, Some(score(3.0, "dependencies", 3)));
    assert!(result.notes.is_empty(), "{:?}", result.notes);
}

#[test]
fn an_or_alternative_free_of_copyleft_is_not_a_license_finding() {
    for (license, copyleft) in [
        ("MIT OR GPL-2.0-only", false),
        ("GPL-2.0-or-later WITH Classpath-exception-2.0", true),
        ("LGPL-2.1-only AND MIT", true),
        ("(MIT OR AGPL-3.0-only) AND SSPL-1.0", true),
        ("Apache-2.0", false),
        ("EUPL-1.2", true),
        ("MPL-2.0", false),
    ] {
        let mut footprint = measured();
        footprint.vulnerabilities = None;
        footprint.dependencies = vec![dependency("x", "1.0.0", Some(license), 1)];
        let (_, findings) = support::run_over(ID, footprint);
        assert_eq!(!findings.is_empty(), copyleft, "{license}");
    }
}

#[test]
fn with_a_base_only_added_or_changed_dependencies_are_reported_and_growth_is() {
    let mut footprint = measured();
    footprint.base = Some(FootprintBase {
        commit: "c0ffee".to_owned(),
        dependencies: vec![
            dependency("serde", "1.0.0", None, 4),
            dependency("gplthing", "0.1.0", None, 9),
        ],
    });
    let (result, findings) = support::run_over(ID, footprint);
    assert_eq!(
        kinds(&findings),
        [("dep.vuln", "smallvec"), ("footprint.grew", "-")]
    );
    let grew = &findings[1];
    assert_eq!(grew.severity, Severity::Warn);
    assert_eq!(
        grew.evidence.0,
        *json!({"base": 2, "head": 3, "added": ["smallvec@0.6.3"], "removed": []})
            .as_object()
            .unwrap()
    );
    assert_eq!(grew.location.as_ref().unwrap().line, 14);
    assert_eq!(
        grew.id,
        FindingId::for_subject(&CheckId::from(ID), "footprint.grew", "dependencies")
    );
    assert_eq!(
        grew.repro,
        "cupel check --only perf.footprint --base c0ffee"
    );
    assert_eq!(result.metrics.get("copyleft"), Some(&0.0));
    assert_eq!(result.metrics.get("vulnerable"), Some(&1.0));
}

#[test]
fn an_upgrade_is_reported_but_is_not_growth() {
    let mut footprint = measured();
    footprint.dependencies = vec![dependency("gplthing", "0.2.0", Some("GPL-3.0-only"), 9)];
    footprint.base = Some(FootprintBase {
        commit: "c0ffee".to_owned(),
        dependencies: vec![dependency("gplthing", "0.1.0", None, 9)],
    });
    let (_, findings) = support::run_over(ID, footprint);
    assert_eq!(kinds(&findings), [("dep.license", "gplthing")]);
}

#[test]
fn what_was_not_measured_is_a_note_and_counts_as_zero() {
    let footprint = Footprint {
        manifests: vec!["go.mod".to_owned()],
        dependencies: vec![dependency("github.com/a/b", "v1.0.0", None, 3)],
        licenses: false,
        vulnerabilities: None,
        artifacts: None,
        base: None,
    };
    let (result, findings) = support::run_over(ID, footprint);
    assert_eq!(findings, []);
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(
        result.notes,
        [
            "licenses not read for these dependencies",
            "vulnerabilities not measured: set `[lang.<id>] advisories` to a directory of OSV advisories",
            "artifact sizes not measured: set `[lang.<id>] artifacts` to their paths or globs",
        ]
    );
    assert_eq!(result.metrics.get("artifact_bytes"), Some(&0.0));
}

#[test]
fn no_manifest_is_a_note_and_scores_zero() {
    let (result, findings) = support::run_over(ID, Footprint::default());
    assert_eq!(findings, []);
    assert_eq!(result.score, Some(score(0.0, "dependencies", 0)));
    assert_eq!(result.notes[0], "no lockfile or manifest found");
}

#[test]
fn settings_are_refused() {
    support::assert_refuses_settings(ID);
}

#[test]
fn every_metric_adds_up_across_languages() {
    support::assert_counts_add_up(
        ID,
        &["dependencies", "vulnerable", "copyleft", "artifact_bytes"],
        "dependencies",
    );
}

#[test]
fn the_manifest_reads_the_footprint() {
    assert_eq!(
        Shape::of(support::check(ID).manifest()),
        Shape {
            needs: vec![FactKind::Footprint],
            wants: vec![],
            tier: 0,
            cost: CostClass::Quick,
            kinds: vec![
                ("footprint.grew", ActionKind::Review),
                ("dep.vuln", ActionKind::FixBug),
                ("dep.license", ActionKind::Review),
            ],
            metrics: vec![
                ("dependencies", MetricPer::Scope),
                ("vulnerable", MetricPer::Scope),
                ("copyleft", MetricPer::Scope),
                ("artifact_bytes", MetricPer::Scope),
            ],
            evidence: vec!["object", "object", "object"],
        }
    );
}
