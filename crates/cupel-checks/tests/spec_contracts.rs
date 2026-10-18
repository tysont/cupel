// ABOUTME: spec.contracts over synthetic ContractViolations facts: one failing finding per violated contract and test,
// ABOUTME: its evidence, identity across line moves, location and repro, notes, and the score (ADR-113).
mod support;

use cupel_core::{
    ActionKind, CheckId, CheckStatus, ContractKind, ContractPlace, ContractViolation,
    ContractViolations, CostClass, FactKind, FindingId, MetricPer, ProducerId, RootRun, RunStatus,
    Severity,
};
use serde_json::json;
use support::{Shape, ran, score};

const ID: &str = "spec.contracts";

fn violation(kind: ContractKind, line: Option<u32>, test: &str) -> ContractViolation {
    ContractViolation {
        kind,
        function: Some("share".to_owned()),
        message: "a share needs at least one part: parts > 0".to_owned(),
        place: line.map(|line| ContractPlace {
            file: "src/lib.rs".to_owned(),
            line,
        }),
        test: test.to_owned(),
    }
}

fn fact(runs: Vec<RootRun>, violations: Vec<ContractViolation>) -> ContractViolations {
    ContractViolations {
        tool: ProducerId::from("tool.rust-contracts"),
        runner: ProducerId::from("tool.cargo-test"),
        declared: true,
        runs,
        tests: 4,
        violations,
    }
}

#[test]
fn a_violated_contract_is_a_failing_finding_at_its_declaration_with_its_test() {
    let (result, findings) = support::run_over(
        ID,
        fact(
            vec![ran(".")],
            vec![violation(
                ContractKind::Precondition,
                Some(4),
                "tests::shares",
            )],
        ),
    );
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(
        (finding.kind.as_str(), finding.severity, finding.action.kind),
        ("contract.violated", Severity::Fail, ActionKind::FixBug)
    );
    assert_eq!(
        finding.title,
        "precondition of `share` violated in `tests::shares`: a share needs at least one part: parts > 0"
    );
    assert_eq!(
        serde_json::to_value(&finding.evidence).unwrap(),
        json!({
            "tool": "tool.rust-contracts",
            "kind": "precondition",
            "function": "share",
            "message": "a share needs at least one part: parts > 0",
            "test": "tests::shares",
        })
    );
    let at = finding.location.as_ref().unwrap();
    assert_eq!(
        (at.file.as_str(), at.line, at.end_line),
        ("src/lib.rs", 4, 4)
    );
    assert_eq!(
        finding.repro,
        "cupel check --only spec.contracts --paths src/lib.rs"
    );
    assert_eq!(finding.action.priority, 1);
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.metrics, [("violations".to_owned(), 1.0)].into());
    assert_eq!(result.score, Some(score(1.0, "violations", 4)));
}

#[test]
fn each_kind_says_whose_obligation_the_contract_was() {
    let (_, findings) = support::run_over(
        ID,
        fact(
            vec![ran(".")],
            vec![
                violation(ContractKind::Precondition, Some(4), "a"),
                violation(ContractKind::Postcondition, Some(10), "b"),
                violation(ContractKind::Invariant, Some(25), "c"),
            ],
        ),
    );
    let summaries: Vec<&str> = findings
        .iter()
        .map(|finding| finding.action.summary.as_str())
        .collect();
    assert_eq!(
        summaries,
        [
            "A caller passed what the precondition rejects: fix the call the test makes, or the \
             precondition if it is wrong, then rerun.",
            "The function returned what its postcondition rejects: fix the function, or the \
             postcondition if it is wrong, then rerun.",
            "A method left the value in a state its invariant rejects: fix the method, or the \
             invariant if it is wrong, then rerun.",
        ]
    );
    assert!(
        findings[1]
            .title
            .starts_with("postcondition of `share` violated")
    );
    assert!(
        findings[2]
            .title
            .starts_with("invariant of `share` violated")
    );
}

#[test]
fn a_violation_keeps_its_id_when_its_lines_move_and_tests_are_told_apart() {
    let id_of = |violations: Vec<ContractViolation>| {
        support::ids_over(ID, fact(vec![ran(".")], violations))
    };
    let at = |line, test| vec![violation(ContractKind::Precondition, Some(line), test)];
    assert_eq!(
        id_of(at(4, "tests::shares")),
        id_of(at(40, "tests::shares"))
    );
    assert_eq!(
        id_of(at(4, "tests::shares")),
        [FindingId::for_subject(
            &CheckId::from(ID),
            "contract.violated",
            "precondition|share|a share needs at least one part: parts > 0|tests::shares"
        )]
    );
    assert_ne!(id_of(at(4, "tests::shares")), id_of(at(4, "tests::splits")));
}

#[test]
fn a_contract_outside_the_repository_has_no_location_and_an_unnamed_one_says_so() {
    let outside = ContractViolation {
        function: None,
        ..violation(ContractKind::Postcondition, None, "tests::pays")
    };
    let (_, findings) = support::run_over(ID, fact(vec![ran(".")], vec![outside]));
    let finding = &findings[0];
    assert_eq!(finding.location, None);
    assert_eq!(finding.repro, "cupel check --only spec.contracts");
    assert_eq!(
        finding.title,
        "postcondition violated in `tests::pays`: a share needs at least one part: parts > 0"
    );
}

#[test]
fn a_failed_run_is_a_note_and_an_undeclared_library_checks_nothing() {
    let failed = RootRun {
        dir: "tools".to_owned(),
        status: RunStatus::Failed,
        detail: Some("error: could not compile `tools`".to_owned()),
    };
    let (result, _) = support::run_over(ID, fact(vec![ran("."), failed], vec![]));
    assert_eq!(
        result.notes,
        [
            "tool.rust-contracts: the contract run of tool.cargo-test failed in tools: error: could not compile `tools`"
        ]
    );
    assert_eq!(result.score, Some(score(0.0, "violations", 4)));
    let undeclared = ContractViolations {
        declared: false,
        runs: vec![],
        tests: 0,
        ..fact(vec![], vec![])
    };
    let (result, findings) = support::run_over(ID, undeclared);
    assert_eq!(findings, []);
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.score, None);
    assert!(result.metrics.is_empty());
    assert_eq!(
        result.notes,
        [
            "tool.rust-contracts: no project root declares the contract library it reads, so no \
          contract was checked"
        ]
    );
}

#[test]
fn settings_are_refused() {
    support::assert_refuses_settings(ID);
}

#[test]
fn counts_add_up_across_languages() {
    support::assert_counts_add_up(ID, &["violations"], "violations");
}

#[test]
fn the_manifest_declares_violated_contracts_over_contract_violations() {
    assert_eq!(
        Shape::of(support::check(ID).manifest()),
        Shape {
            needs: vec![FactKind::ContractViolations],
            wants: vec![],
            tier: 5,
            cost: CostClass::Standard,
            kinds: vec![("contract.violated", ActionKind::FixBug)],
            metrics: vec![("violations", MetricPer::Scope)],
            evidence: vec!["object"],
        }
    );
}
