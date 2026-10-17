// ABOUTME: gen.metamorphic over hand-built RelationResults, with no engine (ADR-098): a violated relation's finding,
// ABOUTME: evidence, identity, location, and repro; held relations; scope; the score and metrics; no settings.
mod support;

use cupel_core::{
    ActionKind, CheckStatus, CostClass, Evaluated, FactKind, Facts, FindingId, Location,
    RelationResult, RelationResults, Scope, Severity, Violation,
};
use serde_json::{Value, json};
use support::{Recorded, check, recorded, score, setting_error};

const ID: &str = "gen.metamorphic";

fn located(file: &str, line: u32) -> Option<Location> {
    Some(Location {
        file: file.to_owned(),
        line,
        col: None,
        end_line: line + 5,
        end_col: None,
    })
}

fn held(name: &str, file: &str) -> RelationResult {
    RelationResult {
        name: name.to_owned(),
        function: format!("{name}:f"),
        input: "int".to_owned(),
        cases: 100,
        location: located(file, 1),
        violation: None,
    }
}

fn violated() -> RelationResult {
    RelationResult {
        name: "sort-reverse".to_owned(),
        function: "sorting:sort_list".to_owned(),
        input: "list[int]".to_owned(),
        cases: 31,
        location: located("sorting.py", 4),
        violation: Some(Violation {
            case: 30,
            input: json!([1, 0, 0]),
            original: json!([12, -7, 3, 3]),
            shrinks: 6,
            transformed: Evaluated::Value(json!([0, 0, 1])),
            source: Evaluated::Value(json!([0, 0, 1])),
            followup: Some(Evaluated::Value(json!([0, 1, 0]))),
            relation_raised: None,
        }),
    }
}

fn run_over(relations: Vec<RelationResult>, scope: Scope) -> (cupel_core::CheckResult, Recorded) {
    let check = check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts
        .insert(RelationResults { seed: 7, relations })
        .unwrap();
    recorded(check, &facts, scope, json!({})).unwrap()
}

fn files(files: &[&str]) -> Scope {
    Scope::of_files(files.iter().map(|file| (*file).to_owned()).collect())
}

#[test]
fn the_manifest_needs_relation_results_at_deep_cost_and_tier_4() {
    let manifest = check(ID).manifest();
    assert_eq!(manifest.needs, [FactKind::RelationResults]);
    assert!(manifest.wants.is_empty());
    assert_eq!(manifest.cost, CostClass::Deep);
    assert_eq!(u8::from(manifest.tier), 4);
    let kinds: Vec<(&str, ActionKind)> = manifest
        .findings
        .iter()
        .map(|spec| (spec.kind.as_str(), spec.action))
        .collect();
    assert_eq!(kinds, [("metamorphic.violated", ActionKind::FixBug)]);
    let metrics: Vec<&str> = manifest.metrics.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(metrics, ["violated", "relations"]);
}

#[test]
fn a_violated_relation_is_one_failing_finding_with_the_minimized_input_and_both_outputs() {
    let (result, recorded) = run_over(vec![violated(), held("add", "arith.py")], Scope::default());

    assert_eq!(recorded.findings.len(), 1);
    let finding = &recorded.findings[0];
    assert_eq!(finding.kind, "metamorphic.violated");
    assert_eq!(
        Value::Object(finding.evidence.0.clone()),
        json!({
            "relation": "sort-reverse",
            "function": "sorting:sort_list",
            "input_type": "list[int]",
            "input": [1, 0, 0],
            "transformed": {"value": [0, 0, 1]},
            "source_output": {"value": [0, 0, 1]},
            "followup_output": {"value": [0, 1, 0]},
            "relation_raised": null,
            "original_input": [12, -7, 3, 3],
            "case": 30,
            "shrinks": 6,
            "seed": 7,
        })
    );
    assert_eq!(
        finding.location,
        located("sorting.py", 4),
        "the function's definition"
    );
    assert_eq!(
        (finding.severity, finding.action.kind, finding.confidence),
        (Severity::Fail, ActionKind::FixBug, 1.0)
    );
    assert_eq!(u8::from(finding.tier), 4);
    assert_eq!(
        finding.title,
        "`sorting:sort_list` breaks the relation `sort-reverse`"
    );
    assert!(
        finding.action.summary.contains("[1,0,0]"),
        "{}",
        finding.action.summary
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.score, Some(score(1.0, "violated", 2)));
    assert_eq!(
        (result.metrics["violated"], result.metrics["relations"]),
        (1.0, 2.0)
    );
}

#[test]
fn findings_are_identified_by_relation_and_function_and_reproduced_with_the_seed() {
    let (_, recorded) = run_over(vec![violated()], Scope::default());
    let finding = &recorded.findings[0];
    assert_eq!(
        finding.id,
        FindingId::for_subject(
            &check(ID).manifest().id,
            "metamorphic.violated",
            "sort-reverse sorting:sort_list"
        )
    );
    assert_eq!(finding.repro, "cupel check --only gen.metamorphic --seed 7");
    assert_eq!(finding.symbol, None);
}

#[test]
fn relations_that_hold_report_nothing_and_pass() {
    let (result, recorded) = run_over(vec![held("a", "a.py"), held("b", "b.py")], Scope::default());
    assert!(recorded.findings.is_empty());
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.score, Some(score(0.0, "violated", 2)));
}

#[test]
fn only_relations_whose_function_is_in_scope_or_unlocated_are_reported() {
    let mut unlocated = violated();
    unlocated.name = "elsewhere".to_owned();
    unlocated.location = None;
    let (result, recorded) = run_over(
        vec![violated(), unlocated, held("a", "a.py")],
        files(&["a.py"]),
    );
    let reported: Vec<&str> = recorded
        .findings
        .iter()
        .map(|f| f.evidence.0["relation"].as_str().unwrap())
        .collect();
    assert_eq!(reported, ["elsewhere"]);
    assert_eq!(result.metrics["relations"], 2.0);
}

#[test]
fn no_relations_is_a_pass_over_none() {
    let (result, recorded) = run_over(Vec::new(), Scope::default());
    assert!(recorded.findings.is_empty());
    assert_eq!(result.score, Some(score(0.0, "violated", 0)));
}

#[test]
fn the_check_takes_no_settings() {
    let (key, valid) = setting_error(check(ID), json!({"cases": 5}));
    assert_eq!((key.as_str(), valid), ("cases", Vec::<String>::new()));
}
