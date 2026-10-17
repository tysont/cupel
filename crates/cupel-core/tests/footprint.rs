// ABOUTME: Tests for the `Footprint` payload (ADR-103): its fact kind and versioned schema, its shape as JSON with
// ABOUTME: every optional measurement absent and present, and how a dependency is spelled.
mod support;

use cupel_core::{Artifact, Dependency, FactKind, Footprint, FootprintBase, Vulnerability};
use serde_json::json;

fn dependency(name: &str, version: Option<&str>) -> Dependency {
    Dependency {
        name: name.to_owned(),
        version: version.map(str::to_owned),
        direct: true,
        license: Some("MIT".to_owned()),
        file: "Cargo.lock".to_owned(),
        line: 7,
    }
}

#[test]
fn footprint_is_an_execution_fact_appended_to_the_catalog_with_a_versioned_schema() {
    let kind = FactKind::Footprint;
    let at = FactKind::ALL.iter().position(|each| *each == kind);
    assert_eq!(
        at.map(|at| FactKind::ALL[at - 1]),
        Some(FactKind::RelationResults)
    );
    assert_eq!(
        support::fact_entry(kind),
        (
            "footprint",
            false,
            Some(1),
            Some(json!("https://cupel.dev/schema/v1/fact/footprint.json")),
        )
    );
}

#[test]
fn a_footprint_serializes_every_field() {
    let measured = Footprint {
        manifests: vec!["Cargo.lock".to_owned()],
        dependencies: vec![dependency("serde", Some("1.0.0"))],
        licenses: true,
        vulnerabilities: Some(vec![Vulnerability {
            id: "RUSTSEC-0000-0001".to_owned(),
            package: "serde".to_owned(),
            version: "1.0.0".to_owned(),
            summary: "Bad".to_owned(),
            fixed: Some("1.0.1".to_owned()),
        }]),
        artifacts: Some(vec![Artifact {
            path: "dist/app".to_owned(),
            bytes: 42,
        }]),
        base: Some(FootprintBase {
            commit: "c0ffee".to_owned(),
            dependencies: Vec::new(),
        }),
    };
    let value = serde_json::to_value(&measured).unwrap();
    assert_eq!(
        value,
        json!({
            "manifests": ["Cargo.lock"],
            "dependencies": [{"name": "serde", "version": "1.0.0", "direct": true,
                              "license": "MIT", "file": "Cargo.lock", "line": 7}],
            "licenses": true,
            "vulnerabilities": [{"id": "RUSTSEC-0000-0001", "package": "serde", "version": "1.0.0",
                                 "summary": "Bad", "fixed": "1.0.1"}],
            "artifacts": [{"path": "dist/app", "bytes": 42}],
            "base": {"commit": "c0ffee", "dependencies": []},
        })
    );
    assert_eq!(
        serde_json::from_value::<Footprint>(value).unwrap(),
        measured
    );
    assert_eq!(
        serde_json::to_value(Footprint::default()).unwrap(),
        json!({"manifests": [], "dependencies": [], "licenses": false,
               "vulnerabilities": null, "artifacts": null, "base": null})
    );
}

#[test]
fn a_dependency_is_spelled_with_its_version_when_it_has_one() {
    assert_eq!(dependency("serde", Some("1.0.0")).spelled(), "serde@1.0.0");
    assert_eq!(dependency("lodash", None).spelled(), "lodash");
}
