// ABOUTME: gen.fuzz over synthetic FuzzResults facts: crashes, timeouts, roots without a target, notes for targets
// ABOUTME: that could not be fuzzed, the score and metrics, and the manifest's shape (ADR-107).
mod support;

use cupel_core::{
    ActionKind, CheckId, CheckResult, CheckStatus, CostClass, FactKind, Facts, Finding, FindingId,
    FuzzCrash, FuzzResults, FuzzStatus, FuzzTarget, Location, MetricPer, ProducerId, Scope,
    Severity,
};
use serde_json::json;
use support::{Shape, score};

const ID: &str = "gen.fuzz";

fn at(file: &str, line: u32) -> Location {
    Location {
        file: file.to_owned(),
        line,
        col: Some(5),
        end_line: line,
        end_col: None,
    }
}

fn target(root: &str, name: &str, status: FuzzStatus) -> FuzzTarget {
    FuzzTarget {
        root: root.to_owned(),
        name: name.to_owned(),
        location: Some(at(&format!("fuzz/fuzz_targets/{name}.rs"), 1)),
        status,
        detail: None,
        executions: Some(1),
        coverage: None,
        crash: None,
    }
}

fn crash(message: &str, location: Option<Location>) -> FuzzCrash {
    FuzzCrash {
        message: message.to_owned(),
        location,
        input: String::new(),
        reproducer: Some(".cupel/cache/fuzz/rust/first/crash-da39".to_owned()),
        reproduce: Some(
            "cargo +nightly fuzz run first .cupel/cache/fuzz/rust/first/crash-da39".to_owned(),
        ),
    }
}

/// Two roots: `.` with a crashing, a hanging, a passing, and a failed target; `tools` with none.
fn fuzzed(seeded: bool) -> FuzzResults {
    let mut crashed = target(".", "first", FuzzStatus::Crashed);
    crashed.crash = Some(crash(
        "index out of bounds: the len is 0 but the index is 0",
        Some(at("src/lib.rs", 5)),
    ));
    let mut hung = target(".", "spin", FuzzStatus::TimedOut);
    hung.crash = Some(FuzzCrash {
        reproducer: Some(".cupel/cache/fuzz/rust/spin/timeout-da39".to_owned()),
        ..crash("timeout after 1 seconds", None)
    });
    let mut failed = target(".", "broken", FuzzStatus::Failed);
    failed.detail = Some("error[E0425]: cannot find value `x`".to_owned());
    FuzzResults {
        tool: ProducerId::from("tool.cargo-fuzz"),
        seed: 3,
        seeded,
        cases: 10_000,
        roots: vec![".".to_owned(), "tools".to_owned()],
        targets: vec![
            crashed,
            hung,
            target(".", "fine", FuzzStatus::Passed),
            failed,
        ],
    }
}

fn run(results: FuzzResults) -> (CheckResult, Vec<Finding>) {
    let check = support::check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(results).unwrap();
    let scope = Scope::of_files(vec!["src/lib.rs".to_owned()]);
    let (result, recorded) = support::recorded(check, &facts, scope, json!({})).unwrap();
    (result, recorded.findings)
}

fn kinds(findings: &[Finding]) -> Vec<&str> {
    findings
        .iter()
        .map(|finding| finding.kind.as_str())
        .collect()
}

#[test]
fn a_crash_is_a_failing_finding_at_the_crash_with_its_reproducer() {
    let (_, findings) = run(fuzzed(true));
    assert_eq!(
        kinds(&findings),
        ["fuzz.crash", "fuzz.timeout", "fuzz.target-missing"]
    );
    let crash = &findings[0];
    assert_eq!(
        (crash.severity, crash.action.kind, crash.confidence),
        (Severity::Fail, ActionKind::FixBug, 1.0)
    );
    assert_eq!(
        crash.id,
        FindingId::for_subject(&CheckId::from(ID), "fuzz.crash", ".|first")
    );
    assert_eq!(crash.location, Some(at("src/lib.rs", 5)));
    assert_eq!(
        crash.title,
        "fuzz target `first` crashes on the empty input: index out of bounds: the len is 0 but \
         the index is 0"
    );
    assert_eq!(
        crash.evidence.0,
        *json!({
            "target": "first",
            "tool": "tool.cargo-fuzz",
            "message": "index out of bounds: the len is 0 but the index is 0",
            "input": "",
            "reproducer": ".cupel/cache/fuzz/rust/first/crash-da39",
            "reproduce": "cargo +nightly fuzz run first .cupel/cache/fuzz/rust/first/crash-da39",
            "seed": 3,
            "cases": 10000,
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(
        crash.repro,
        "cupel check --only gen.fuzz --paths fuzz/fuzz_targets/first.rs --seed 3 --cases 10000"
    );
    assert!(
        crash
            .action
            .summary
            .contains("cargo +nightly fuzz run first .cupel/cache/fuzz/rust/first/crash-da39"),
        "{}",
        crash.action.summary
    );
}

#[test]
fn a_timeout_is_a_warning_at_the_target_when_the_hang_names_no_place() {
    let (_, findings) = run(fuzzed(true));
    let timeout = &findings[1];
    assert_eq!(
        (timeout.severity, timeout.action.kind),
        (Severity::Warn, ActionKind::FixBug)
    );
    assert_eq!(timeout.location, Some(at("fuzz/fuzz_targets/spin.rs", 1)));
    assert_eq!(
        timeout.title,
        "fuzz target `spin` runs past its time limit on the empty input: timeout after 1 seconds"
    );
    assert_eq!(
        timeout.id,
        FindingId::for_subject(&CheckId::from(ID), "fuzz.timeout", ".|spin")
    );
}

#[test]
fn a_root_without_a_target_is_an_information_finding() {
    let (_, findings) = run(fuzzed(true));
    let missing = &findings[2];
    assert_eq!(
        (missing.severity, missing.action.kind),
        (Severity::Info, ActionKind::WriteTest)
    );
    assert_eq!(missing.location, None);
    assert_eq!(missing.title, "no fuzz target in `tools`");
    assert_eq!(
        missing.evidence.0,
        *json!({"root": "tools", "tool": "tool.cargo-fuzz"})
            .as_object()
            .unwrap()
    );
    assert_eq!(
        missing.id,
        FindingId::for_subject(&CheckId::from(ID), "fuzz.target-missing", "tools")
    );
    assert_eq!(missing.repro, "cupel check --only gen.fuzz --paths tools");
}

#[test]
fn an_unseeded_fuzzer_reports_no_seed_and_a_printable_input_is_quoted() {
    let mut results = fuzzed(false);
    if let Some(crash) = results.targets[0].crash.as_mut() {
        crash.input = "FUZ".to_owned();
        crash.location = None;
    }
    results.targets[0].location = None;
    let (_, findings) = run(results);
    let crash = &findings[0];
    assert_eq!(crash.evidence.0["seed"], json!(null));
    assert_eq!(crash.location, None);
    assert_eq!(
        crash.repro,
        "cupel check --only gen.fuzz --paths . --cases 10000"
    );
    assert!(crash.title.contains("crashes on `FUZ`"), "{}", crash.title);
}

#[test]
fn the_score_counts_failing_targets_and_failed_targets_are_notes() {
    let (result, _) = run(fuzzed(true));
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.score, Some(score(2.0, "failing", 4)));
    assert_eq!(
        result.metrics,
        [
            ("crashes".to_owned(), 1.0),
            ("missing".to_owned(), 1.0),
            ("targets".to_owned(), 4.0),
            ("timeouts".to_owned(), 1.0),
        ]
        .into()
    );
    assert_eq!(
        result.notes,
        ["broken could not be fuzzed: error[E0425]: cannot find value `x`"]
    );
}

#[test]
fn targets_that_all_pass_pass_the_check() {
    let results = FuzzResults {
        roots: vec![".".to_owned()],
        targets: vec![target(".", "fine", FuzzStatus::Passed)],
        ..fuzzed(true)
    };
    let (result, findings) = run(results);
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(result.score, Some(score(0.0, "failing", 1)));
}

#[test]
fn settings_are_refused() {
    support::assert_refuses_settings(ID);
}

#[test]
fn every_metric_adds_up_across_languages() {
    support::assert_counts_add_up(
        ID,
        &["targets", "crashes", "timeouts", "missing"],
        "failing",
    );
}

#[test]
fn the_manifest_reads_the_fuzz_results() {
    assert_eq!(
        Shape::of(support::check(ID).manifest()),
        Shape {
            needs: vec![FactKind::FuzzResults],
            wants: vec![],
            tier: 4,
            cost: CostClass::Deep,
            kinds: vec![
                ("fuzz.crash", ActionKind::FixBug),
                ("fuzz.timeout", ActionKind::FixBug),
                ("fuzz.target-missing", ActionKind::WriteTest),
            ],
            metrics: vec![
                ("targets", MetricPer::Scope),
                ("crashes", MetricPer::Scope),
                ("timeouts", MetricPer::Scope),
                ("missing", MetricPer::Scope),
            ],
            evidence: vec!["object", "object", "object"],
        }
    );
}
