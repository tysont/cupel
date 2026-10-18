// ABOUTME: conc.race over synthetic RaceReports facts: one finding per data race or undefined behavior, its evidence,
// ABOUTME: identity across line moves, location and repro, notes for runs that failed, and the score (ADR-105).
mod support;

use cupel_core::{
    ActionKind, CheckId, CheckStatus, CostClass, FactKind, FindingId, MetricPer, ProducerId,
    RaceFrame, RaceKind, RaceReport, RaceReports, RaceStack, RootRun, RunStatus, Severity,
};
use serde_json::json;
use support::{Shape, ran, score};

const ID: &str = "conc.race";

fn frame(function: Option<&str>, file: &str, line: u32) -> RaceFrame {
    RaceFrame {
        function: function.map(str::to_owned),
        file: file.to_owned(),
        line,
    }
}

fn stack(access: Option<&str>, frames: Vec<RaceFrame>) -> RaceStack {
    RaceStack {
        access: access.map(str::to_owned),
        frames,
    }
}

/// A data race between a write and a read of `units::count`, at `line`.
fn race(line: u32) -> RaceReport {
    RaceReport {
        kind: RaceKind::DataRace,
        test: Some("counts".to_owned()),
        message: "data race".to_owned(),
        stacks: vec![
            stack(
                Some("read"),
                vec![frame(Some("units::count"), "src/lib.rs", line)],
            ),
            stack(
                Some("write"),
                vec![frame(Some("units::count"), "src/lib.rs", line)],
            ),
        ],
    }
}

fn undefined(message: &str) -> RaceReport {
    RaceReport {
        kind: RaceKind::UndefinedBehavior,
        test: Some("reads_last".to_owned()),
        message: message.to_owned(),
        stacks: vec![stack(
            None,
            vec![
                frame(Some("units::last"), "src/lib.rs", 26),
                frame(Some("reads_last"), "tests/ub.rs", 3),
            ],
        )],
    }
}

fn reports(runs: Vec<RootRun>, reports: Vec<RaceReport>) -> RaceReports {
    RaceReports {
        tool: ProducerId::from("tool.miri"),
        runs,
        reports,
    }
}

#[test]
fn a_data_race_is_a_failing_finding_at_its_first_access_with_the_stacks_as_evidence() {
    let (result, findings) = support::run_over(ID, reports(vec![ran(".")], vec![race(12)]));
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(
        (finding.kind.as_str(), finding.severity, finding.action.kind),
        ("race.detected", Severity::Fail, ActionKind::FixBug)
    );
    assert_eq!(
        finding.title,
        "data race: `read` in `units::count` and `write` in `units::count`"
    );
    assert_eq!(
        serde_json::to_value(&finding.evidence).unwrap(),
        json!({
            "tool": "tool.miri",
            "test": "counts",
            "message": "data race",
            "stacks": [
                {"access": "read", "frames": [{"function": "units::count", "file": "src/lib.rs", "line": 12}]},
                {"access": "write", "frames": [{"function": "units::count", "file": "src/lib.rs", "line": 12}]},
            ],
        })
    );
    let at = finding.location.as_ref().unwrap();
    assert_eq!(
        (at.file.as_str(), at.line, at.end_line),
        ("src/lib.rs", 12, 12)
    );
    assert_eq!(
        finding.repro,
        "cupel check --only conc.race --paths src/lib.rs"
    );
    assert_eq!(finding.action.priority, 1);
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(
        result.metrics,
        [
            ("races".to_owned(), 1.0),
            ("undefined_behavior".to_owned(), 0.0)
        ]
        .into()
    );
    assert_eq!(result.score, Some(score(1.0, "reports", 1)));
}

#[test]
fn undefined_behavior_is_a_failing_finding_titled_by_its_message() {
    let message = "memory access failed: attempting to access 1 byte, but got alloc+0x4";
    let (result, findings) =
        support::run_over(ID, reports(vec![ran(".")], vec![undefined(message)]));
    let finding = &findings[0];
    assert_eq!(
        (finding.kind.as_str(), finding.severity),
        ("ub.detected", Severity::Fail)
    );
    assert_eq!(
        finding.title,
        format!("undefined behavior in `units::last`: {message}")
    );
    assert_eq!(finding.location.as_ref().unwrap().line, 26);
    assert_eq!(finding.evidence.0["test"], json!("reads_last"));
    assert_eq!(
        result.metrics,
        [
            ("races".to_owned(), 0.0),
            ("undefined_behavior".to_owned(), 1.0)
        ]
        .into()
    );
}

#[test]
fn a_report_keeps_its_id_when_its_lines_move_and_repeats_are_told_apart() {
    let id_of = |report: Vec<RaceReport>| support::ids_over(ID, reports(vec![ran(".")], report));
    assert_eq!(id_of(vec![race(12)]), id_of(vec![race(40)]));
    let check = CheckId::from(ID);
    assert_eq!(
        id_of(vec![race(12)]),
        [FindingId::for_subject(
            &check,
            "race.detected",
            "counts|data race|units::count|units::count|1"
        )]
    );
    let twice = id_of(vec![race(12), race(30)]);
    assert_eq!(twice.len(), 2);
    assert_ne!(twice[0], twice[1]);
}

#[test]
fn a_report_without_frames_has_no_location_and_names_no_function() {
    let bare = RaceReport {
        stacks: vec![stack(Some("write"), vec![]), stack(Some("read"), vec![])],
        test: None,
        ..race(1)
    };
    let (_, findings) = support::run_over(ID, reports(vec![ran(".")], vec![bare]));
    let finding = &findings[0];
    assert_eq!(finding.location, None);
    assert_eq!(finding.repro, "cupel check --only conc.race");
    assert_eq!(
        finding.title,
        "data race: `write` in an unnamed function and `read` in an unnamed function"
    );
    let unnamed = RaceReport {
        stacks: vec![stack(None, vec![frame(None, "src/lib.rs", 12)])],
        ..undefined("bad")
    };
    let (_, findings) = support::run_over(ID, reports(vec![ran(".")], vec![unnamed]));
    assert_eq!(findings[0].title, "undefined behavior: bad");
}

#[test]
fn a_failed_run_is_a_note_beside_the_reports_of_the_others() {
    let failed = RootRun {
        dir: "tools".to_owned(),
        status: RunStatus::Failed,
        detail: Some("error: could not compile `tools`".to_owned()),
    };
    let (result, findings) = support::run_over(ID, reports(vec![ran("."), failed], vec![race(12)]));
    assert_eq!(findings.len(), 1);
    assert_eq!(
        result.notes,
        ["tool.miri failed in tools: error: could not compile `tools`"]
    );
    assert_eq!(result.score, Some(score(1.0, "reports", 1)));
}

#[test]
fn nothing_reported_passes_and_no_run_scores_nothing() {
    let (clean, findings) = support::run_over(ID, reports(vec![ran(".")], vec![]));
    assert_eq!(findings, []);
    assert_eq!(clean.status, CheckStatus::Pass);
    assert_eq!(clean.score, Some(score(0.0, "reports", 1)));
    let (none, _) = support::run_over(ID, reports(vec![], vec![]));
    assert_eq!(none.score, None);
    assert!(none.metrics.is_empty());
}

#[test]
fn settings_are_refused() {
    support::assert_refuses_settings(ID);
}

#[test]
fn counts_add_up_across_languages() {
    support::assert_counts_add_up(ID, &["races", "undefined_behavior"], "reports");
}

#[test]
fn the_manifest_declares_races_and_undefined_behavior_over_race_reports() {
    assert_eq!(
        Shape::of(support::check(ID).manifest()),
        Shape {
            needs: vec![FactKind::RaceReports],
            wants: vec![],
            tier: 4,
            cost: CostClass::Standard,
            kinds: vec![
                ("race.detected", ActionKind::FixBug),
                ("ub.detected", ActionKind::FixBug)
            ],
            metrics: vec![
                ("races", MetricPer::Scope),
                ("undefined_behavior", MetricPer::Scope)
            ],
            evidence: vec!["object", "object"],
        }
    );
}
