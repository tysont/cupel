// ABOUTME: tests.mutation over a hand-built MutationOutcomes fact, with no engine: findings per outcome, the score, ADR-004's
// ABOUTME: possibly equivalent survivors, ADR-082's strategies and fallbacks, scope, IDs, and combining languages.
mod support;

use cupel_core::syntax::{
    MutationOperator, Span, SymbolDef, SymbolKind, Symbols, TestConventions, TestPairing,
    Visibility,
};
use cupel_core::{
    ActionKind, CheckResult, CheckStatus, CostClass, Error, ExcludedTest, FactKind, Facts, Finding,
    LangId, MetricPer, Mutant, MutantOutcome, MutationOutcomes, MutationPhase, MutationStrategy,
    PhaseDuration, ProducerId, ProtectionMetric, Scope, Severity,
};
use serde_json::{Value, json};
use support::{Recorded, Shape, combined, language};

const ID: &str = "tests.mutation";

/// `discount` in `discount.py`, lines 4–8.
fn discount() -> SymbolDef {
    SymbolDef {
        name: "discount".to_owned(),
        namespace: "discount".to_owned(),
        display: "discount.discount".to_owned(),
        kind: SymbolKind::Function,
        visibility: Visibility::Public,
        lang: LangId::from("python"),
        file: "discount.py".to_owned(),
        span: Span {
            line: 4,
            col: 1,
            end_line: 8,
            end_col: 21,
        },
        body: None,
        parameters: 2,
    }
}

/// A mutant of `discount` replacing `from` with `to` at `line`, column `col`.
fn mutant(line: u32, col: u32, from: &str, to: &str, outcome: MutantOutcome) -> Mutant {
    Mutant {
        symbol: discount().key(),
        symbol_hash: Some(support::hash_of_line(4)),
        site_hash: format!("{:016x}", line * 100 + col).parse().unwrap(),
        operator: MutationOperator::Relational,
        from: from.to_owned(),
        to: to.to_owned(),
        span: Span {
            line,
            col,
            end_line: line,
            end_col: col + u32::try_from(from.len()).unwrap(),
        },
        outcome,
        tests: vec!["tests/test_large.py::test_large".to_owned()],
        killing_tests: Vec::new(),
        duration_ms: Some(120),
        reused: false,
        equivalent: None,
        cache_key: None,
        strategy: None,
        fallback: None,
    }
}

/// Facts with `discount`, outcomes of `mutants`, and test conventions pairing its file.
fn facts(mutants: Vec<Mutant>) -> Facts {
    let check = support::check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts
        .insert(Symbols {
            symbols: vec![discount()],
        })
        .unwrap();
    facts
        .insert(MutationOutcomes::new(
            ProducerId::from("engine.test"),
            Some(ProducerId::from("tool.pytest")),
            CostClass::Standard,
            false,
            mutants,
        ))
        .unwrap();
    facts
        .insert(TestConventions {
            test_globs: vec!["tests/**".to_owned()],
            pairs: vec![TestPairing {
                source: "discount.py".to_owned(),
                tests: vec!["tests/test_large.py".to_owned()],
            }],
        })
        .unwrap();
    facts
}

fn scope() -> Scope {
    Scope {
        files: vec!["discount.py".to_owned()],
        symbols: Some([discount().key()].into()),
    }
}

fn run(mutants: Vec<Mutant>) -> (CheckResult, Recorded) {
    support::recorded(support::check(ID), &facts(mutants), scope(), json!({})).unwrap()
}

/// Each finding's kind, severity, and confidence.
fn kinds(findings: &[Finding]) -> Vec<(&str, Severity, f64)> {
    findings
        .iter()
        .map(|f| (f.kind.as_str(), f.severity, f.confidence))
        .collect()
}

#[test]
fn the_manifest_declares_its_facts_kinds_actions_and_metrics() {
    let check = support::check(ID);
    let shape = Shape::of(check.manifest());
    assert_eq!(
        (shape.needs, shape.wants, shape.tier, shape.cost),
        (
            vec![FactKind::Symbols, FactKind::MutationOutcomes],
            vec![FactKind::TestConventions],
            3,
            CostClass::Standard
        )
    );
    assert_eq!(
        shape.kinds,
        [
            ("mutant.survived", ActionKind::WriteTest),
            ("mutant.timeout", ActionKind::Review),
            ("mutant.out-of-memory", ActionKind::Review),
            ("mutant.uncovered", ActionKind::WriteTest),
            ("mutant.equivalent?", ActionKind::Review),
            ("mutant.slow-test-excluded", ActionKind::Review),
        ]
    );
    assert_eq!(
        shape.metrics,
        [
            ("score", MetricPer::Scope),
            ("killed", MetricPer::Scope),
            ("survived", MetricPer::Scope),
            ("timeout", MetricPer::Scope),
            ("out_of_memory", MetricPer::Scope),
            ("uncovered", MetricPer::Scope),
            ("unviable", MetricPer::Scope),
            ("equivalent", MetricPer::Scope),
            ("executed", MetricPer::Scope),
            ("reused", MetricPer::Scope),
            ("schemata", MetricPer::Scope),
            ("fallback", MetricPer::Scope),
            ("plan_ms", MetricPer::Scope),
            ("sync_ms", MetricPer::Scope),
            ("rewrite_ms", MetricPer::Scope),
            ("build_ms", MetricPer::Scope),
            ("run_ms", MetricPer::Scope),
            ("validate_ms", MetricPer::Scope),
            ("survivors", MetricPer::Symbol),
            ("mutation_score", MetricPer::Symbol),
            ("tests", MetricPer::Symbol),
        ]
    );
    let protection: Vec<(&str, Option<ProtectionMetric>)> = check
        .manifest()
        .metrics
        .iter()
        .filter(|metric| metric.protection.is_some())
        .map(|metric| (metric.name.as_str(), metric.protection))
        .collect();
    assert_eq!(
        protection,
        [
            ("mutation_score", Some(ProtectionMetric::MutationScore)),
            ("tests", Some(ProtectionMetric::Tests)),
        ]
    );
}

#[test]
fn a_surviving_mutant_is_a_failing_finding_at_its_site_naming_the_tests_that_missed_it() {
    let (result, recorded) = run(vec![mutant(6, 14, ">=", ">", MutantOutcome::Survived)]);
    let finding = &recorded.findings[0];
    assert_eq!(
        kinds(&recorded.findings),
        [("mutant.survived", Severity::Fail, 0.95)]
    );
    assert_eq!(
        finding.title,
        "Mutant survived: `>=` → `>` at discount.py:6"
    );
    let location = finding.location.as_ref().unwrap();
    assert_eq!(
        (location.file.as_str(), location.line, location.col),
        ("discount.py", 6, Some(14))
    );
    assert_eq!(
        Value::Object(finding.evidence.0.clone()),
        json!({
            "mutation": {"from": ">=", "to": ">", "operator": "relational"},
            "covering_tests": ["tests/test_large.py::test_large"],
            "tests_run": 1
        })
    );
    assert_eq!(
        (
            finding.action.kind,
            finding.action.hints.get("nearest_test_file"),
            finding.action.hints.get("covering_tests")
        ),
        (
            ActionKind::WriteTest,
            Some(&json!("tests/test_large.py")),
            Some(&json!(["tests/test_large.py::test_large"]))
        )
    );
    assert_eq!(
        finding.symbol.as_ref().unwrap().display,
        "discount.discount"
    );
    assert_eq!(result.status, CheckStatus::Fail);
}

#[test]
fn the_score_is_killed_over_killed_surviving_and_uncovered() {
    let (result, _) = run(vec![
        mutant(6, 14, ">=", ">", MutantOutcome::Survived),
        mutant(6, 20, "and", "or", MutantOutcome::Killed),
        mutant(7, 22, "-", "+", MutantOutcome::Killed),
        mutant(7, 24, "10", "11", MutantOutcome::Timeout),
        mutant(8, 12, "*", "/", MutantOutcome::Uncovered),
        mutant(8, 14, "1", "0", MutantOutcome::Unviable),
    ]);
    let score = result.score.unwrap();
    assert_eq!((score.value, score.n), (2.0 / 4.0, 4));
    let metric = |name: &str| result.metrics.get(name).copied();
    assert_eq!(
        [
            "killed",
            "survived",
            "timeout",
            "uncovered",
            "unviable",
            "executed"
        ]
        .map(metric),
        [
            Some(2.0),
            Some(1.0),
            Some(1.0),
            Some(1.0),
            Some(1.0),
            Some(5.0)
        ]
    );
}

/// `mutant` built by `strategy`, falling back for `fallback`.
fn built(mut mutant: Mutant, strategy: MutationStrategy, fallback: Option<&str>) -> Mutant {
    mutant.strategy = Some(strategy);
    mutant.fallback = fallback.map(str::to_owned);
    mutant
}

#[test]
fn the_result_counts_mutants_run_as_schemata_and_notes_each_fallback() {
    let pattern = "inside a pattern";
    let failed = "the schemata build of `discount.rs` failed: error[E0369]";
    let mut reused = built(
        mutant(8, 12, "*", "/", MutantOutcome::Killed),
        MutationStrategy::FileCopy,
        Some(failed),
    );
    reused.reused = true;
    let (result, _) = run(vec![
        built(
            mutant(6, 14, ">=", ">", MutantOutcome::Killed),
            MutationStrategy::Schemata,
            None,
        ),
        built(
            mutant(6, 20, "and", "or", MutantOutcome::Killed),
            MutationStrategy::FileCopy,
            Some(pattern),
        ),
        built(
            mutant(7, 22, "-", "+", MutantOutcome::Killed),
            MutationStrategy::FileCopy,
            Some(failed),
        ),
        built(
            mutant(7, 24, "10", "11", MutantOutcome::Killed),
            MutationStrategy::FileCopy,
            Some(failed),
        ),
        built(
            mutant(8, 14, "1", "0", MutantOutcome::Killed),
            MutationStrategy::FileCopy,
            None,
        ),
        reused,
    ]);
    let metric = |name: &str| result.metrics.get(name).copied();
    assert_eq!(
        ["schemata", "fallback", "executed"].map(metric),
        [Some(1.0), Some(3.0), Some(5.0)]
    );
    assert_eq!(
        result.notes,
        [
            "1 mutant ran as a file copy: inside a pattern".to_owned(),
            format!("2 mutants ran as file copies: {failed}"),
        ]
    );
}

/// A phase row of `phase` that took `ms`.
fn phase(phase: MutationPhase, slot: Option<u32>, ms: u64) -> PhaseDuration {
    PhaseDuration {
        phase,
        file: Some("discount.py".to_owned()),
        slot,
        tests: None,
        duration_ms: ms,
    }
}

#[test]
fn each_phase_of_the_engines_work_is_reported_as_its_total_in_milliseconds() {
    let check = support::check(ID);
    let mut facts = facts(vec![mutant(6, 14, ">=", ">", MutantOutcome::Killed)]);
    let mut outcomes = facts.get::<MutationOutcomes>().unwrap().clone();
    outcomes.phases = vec![
        phase(MutationPhase::Plan, None, 40),
        phase(MutationPhase::Sync, Some(0), 300),
        phase(MutationPhase::Sync, Some(1), 200),
        phase(MutationPhase::Build, Some(0), 5000),
        phase(MutationPhase::Build, Some(1), 4000),
        phase(MutationPhase::Run, Some(0), 1500),
    ];
    facts.insert(outcomes).unwrap();
    let (result, _) = support::recorded(check, &facts, scope(), json!({})).unwrap();
    let metric = |name: &str| result.metrics.get(name).copied();
    assert_eq!(
        [
            "plan_ms",
            "sync_ms",
            "rewrite_ms",
            "build_ms",
            "run_ms",
            "validate_ms"
        ]
        .map(metric),
        [
            Some(40.0),
            Some(500.0),
            Some(0.0),
            Some(9000.0),
            Some(1500.0),
            Some(0.0)
        ]
    );
}

#[test]
fn timeouts_and_uncovered_mutants_are_notes_and_unviable_ones_are_not_reported() {
    let (result, recorded) = run(vec![
        mutant(7, 24, "10", "11", MutantOutcome::Timeout),
        mutant(8, 12, "*", "/", MutantOutcome::Uncovered),
        mutant(8, 14, "1", "0", MutantOutcome::Unviable),
    ]);
    assert_eq!(
        kinds(&recorded.findings),
        [
            ("mutant.timeout", Severity::Info, 1.0),
            ("mutant.uncovered", Severity::Info, 1.0)
        ]
    );
    assert_eq!(result.status, CheckStatus::Pass);
    let score = result.score.unwrap();
    assert_eq!(
        (score.value, score.n),
        (0.0, 1),
        "the uncovered mutant is missed"
    );
}

#[test]
fn a_mutant_killed_over_its_memory_budget_is_a_note_left_out_of_the_score() {
    let (result, recorded) = run(vec![
        mutant(7, 24, "10", "11", MutantOutcome::OutOfMemory),
        mutant(8, 12, "*", "/", MutantOutcome::Killed),
    ]);
    assert_eq!(
        kinds(&recorded.findings),
        [("mutant.out-of-memory", Severity::Info, 1.0)]
    );
    let finding = &recorded.findings[0];
    assert_eq!(finding.evidence.0["mutation"]["to"], json!("11"));
    assert!(
        finding.action.summary.contains("max_mutant_memory"),
        "{}",
        finding.action.summary
    );
    let score = result.score.unwrap();
    assert_eq!((score.value, score.n), (1.0, 1));
    assert_eq!(result.metrics.get("out_of_memory"), Some(&1.0));
    assert_eq!(result.status, CheckStatus::Pass);
}

/// Timeouts and unviable mutants are left out of the score, so a scope of only those has
/// nothing scored and scores 1.
#[test]
fn a_scope_of_only_timeouts_and_unviable_mutants_scores_one() {
    let (result, _) = run(vec![
        mutant(7, 24, "10", "11", MutantOutcome::Timeout),
        mutant(8, 14, "1", "0", MutantOutcome::Unviable),
    ]);
    let score = result.score.unwrap();
    assert_eq!((score.value, score.n), (1.0, 0));
}

#[test]
fn a_survivor_marked_possibly_equivalent_is_a_low_confidence_note_left_out_of_the_score() {
    let mut equivalent = mutant(8, 18, "*", "/", MutantOutcome::Survived);
    equivalent.equivalent = Some("the right operand is `1`".to_owned());
    let mut killed_anyway = mutant(8, 20, "*", "/", MutantOutcome::Killed);
    killed_anyway.equivalent = Some("the right operand is `1`".to_owned());
    let (result, recorded) = run(vec![equivalent, killed_anyway]);
    assert_eq!(
        kinds(&recorded.findings),
        [("mutant.equivalent?", Severity::Info, 0.3)]
    );
    assert_eq!(
        recorded.findings[0].evidence.0["reason"],
        json!("the right operand is `1`")
    );
    let score = result.score.unwrap();
    assert_eq!((score.value, score.n), (1.0, 1));
    assert_eq!(result.metrics.get("equivalent"), Some(&1.0));
    assert_eq!(result.status, CheckStatus::Pass);
}

#[test]
fn every_replacement_of_a_site_has_its_own_id_and_a_moved_site_keeps_it() {
    let (_, recorded) = run(vec![
        mutant(6, 14, ">=", ">", MutantOutcome::Survived),
        mutant(6, 14, ">=", "<", MutantOutcome::Survived),
    ]);
    assert_ne!(recorded.findings[0].id, recorded.findings[1].id);
    let mut moved = mutant(6, 14, ">=", ">", MutantOutcome::Survived);
    moved.span.line = 16;
    let (_, again) = run(vec![moved]);
    let greater = recorded
        .findings
        .iter()
        .find(|finding| finding.evidence.0["mutation"]["to"] == json!(">"))
        .unwrap();
    assert_eq!(again.findings[0].id, greater.id);
}

#[test]
fn only_mutants_of_symbols_in_scope_are_reported_and_ranked() {
    let check = support::check(ID);
    let outside = Scope {
        files: vec!["discount.py".to_owned()],
        symbols: Some(std::collections::BTreeSet::new()),
    };
    let facts = facts(vec![mutant(6, 14, ">=", ">", MutantOutcome::Survived)]);
    let (result, recorded) = support::recorded(check, &facts, outside, json!({})).unwrap();
    assert!(recorded.findings.is_empty());
    assert_eq!(result.score.unwrap().n, 0);
    let (_, inside) = run(vec![mutant(6, 14, ">=", ">", MutantOutcome::Survived)]);
    assert_eq!(
        inside.measured(),
        [
            ("survivors", "discount.discount", 1.0),
            ("mutation_score", "discount.discount", 0.0),
            ("tests", "discount.discount", 1.0),
        ]
    );
}

#[test]
fn each_function_is_measured_for_its_killed_ratio_and_the_tests_run_against_it() {
    let mut also_small = mutant(6, 14, ">=", "<", MutantOutcome::Killed);
    also_small
        .tests
        .push("tests/test_small.py::test_small".to_owned());
    let mut equivalent = mutant(8, 12, "*", "/", MutantOutcome::Survived);
    equivalent.equivalent = Some("multiplies by one".to_owned());
    let (_, recorded) = run(vec![
        mutant(6, 14, ">=", ">", MutantOutcome::Survived),
        also_small,
        mutant(7, 16, "-", "+", MutantOutcome::Killed),
        mutant(7, 9, "return", "pass", MutantOutcome::Timeout),
        equivalent,
    ]);
    assert_eq!(
        recorded.measured(),
        [
            ("survivors", "discount.discount", 1.0),
            ("mutation_score", "discount.discount", 2.0 / 3.0),
            ("tests", "discount.discount", 2.0),
        ]
    );
}

/// A mutant of `discount` with `outcome` that no test ran against.
fn untested(outcome: MutantOutcome) -> Mutant {
    let mut mutant = mutant(6, 14, ">=", ">", outcome);
    mutant.tests.clear();
    mutant
}

#[test]
fn a_function_with_no_scored_mutant_has_no_killed_ratio() {
    let (_, recorded) = run(vec![untested(MutantOutcome::Timeout)]);
    assert_eq!(
        recorded.measured(),
        [
            ("survivors", "discount.discount", 0.0),
            ("tests", "discount.discount", 0.0),
        ]
    );
}

/// A function no test runs is unprotected: its uncovered mutants are missed, so it scores 0
/// and ranks first for protection (ADR-132).
#[test]
fn a_function_whose_mutants_no_test_runs_has_a_killed_ratio_of_zero() {
    let (_, recorded) = run(vec![untested(MutantOutcome::Uncovered)]);
    assert_eq!(
        recorded.measured(),
        [
            ("survivors", "discount.discount", 0.0),
            ("mutation_score", "discount.discount", 0.0),
            ("tests", "discount.discount", 0.0),
        ]
    );
}

#[test]
fn without_mutation_outcomes_the_check_cannot_run() {
    let check = support::check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts
        .insert(Symbols {
            symbols: vec![discount()],
        })
        .unwrap();
    let Err(error) = support::recorded(check, &facts, scope(), json!({})) else {
        panic!("the check ran without its outcomes")
    };
    assert_eq!(
        error,
        Error::AbsentFact {
            kind: FactKind::MutationOutcomes
        }
    );
}

/// The check's settings, as a configuration error lists them.
fn settings() -> Vec<String> {
    vec![
        "max_mutant_timeout".to_owned(),
        "max_mutant_memory".to_owned(),
    ]
}

#[test]
fn the_cap_and_the_budget_of_a_mutants_run_are_its_settings_and_configure_the_engines_fact() {
    assert_eq!(
        support::declared_settings(ID),
        [
            ("max_mutant_timeout", json!("120s")),
            ("max_mutant_memory", json!("50%"))
        ]
    );
    let configures: Vec<Option<FactKind>> = support::check(ID)
        .manifest()
        .settings
        .iter()
        .map(|setting| setting.configures)
        .collect();
    assert_eq!(configures, [Some(FactKind::MutationOutcomes); 2]);
}

#[test]
fn any_setting_is_a_configuration_error() {
    let (key, valid) = support::setting_error(support::check(ID), json!({"timeout_factor": 10}));
    assert_eq!((key.as_str(), valid), ("timeout_factor", settings()));
}

#[test]
fn a_cap_that_is_not_a_duration_is_a_configuration_error() {
    for (config, refused) in [
        (json!({"max_mutant_timeout": 120}), "max_mutant_timeout"),
        (
            json!({"max_mutant_timeout": "two minutes"}),
            "max_mutant_timeout",
        ),
    ] {
        let (key, valid) = support::setting_error(support::check(ID), config);
        assert_eq!((key.as_str(), valid), (refused, settings()));
    }
    let (ctx, _dir) = support::ctx(scope(), json!({"max_mutant_timeout": "90s"}));
    assert!(support::check(ID).plan(&ctx, &scope()).is_ok());
}

#[test]
fn a_budget_that_is_neither_a_size_nor_a_share_is_a_configuration_error() {
    for config in [
        json!({"max_mutant_memory": 3}),
        json!({"max_mutant_memory": "3 gigabytes"}),
        json!({"max_mutant_memory": "0%"}),
    ] {
        let (key, valid) = support::setting_error(support::check(ID), config);
        assert_eq!((key.as_str(), valid), ("max_mutant_memory", settings()));
    }
    let (ctx, _dir) = support::ctx(scope(), json!({"max_mutant_memory": "3g"}));
    assert!(support::check(ID).plan(&ctx, &scope()).is_ok());
}

#[test]
fn each_slow_test_left_out_of_whole_suite_runs_is_one_info_finding_naming_its_duration() {
    let mut facts = facts(vec![mutant(6, 14, ">=", ">", MutantOutcome::Killed)]);
    let mut outcomes = MutationOutcomes::new(
        ProducerId::from("engine.test"),
        Some(ProducerId::from("tool.pytest")),
        CostClass::Standard,
        false,
        vec![mutant(6, 14, ">=", ">", MutantOutcome::Killed)],
    );
    outcomes.excluded = vec![ExcludedTest {
        id: "tests/test_slow.py::test_slow".to_owned(),
        file: Some("tests/test_slow.py".to_owned()),
        baseline_ms: 61_300,
    }];
    facts.insert(outcomes).unwrap();
    let (result, recorded) =
        support::recorded(support::check(ID), &facts, scope(), json!({})).unwrap();
    let excluded: Vec<&Finding> = recorded
        .findings
        .iter()
        .filter(|finding| finding.kind == "mutant.slow-test-excluded")
        .collect();
    assert_eq!(excluded.len(), 1);
    let finding = excluded[0];
    assert_eq!(
        (
            finding.severity,
            finding.action.kind,
            finding.title.as_str()
        ),
        (
            Severity::Info,
            ActionKind::Review,
            "Slow test left out of mutant runs: `tests/test_slow.py::test_slow` (61.3 s in the baseline)"
        )
    );
    assert_eq!(
        Value::Object(finding.evidence.0.clone()),
        json!({
            "test": "tests/test_slow.py::test_slow",
            "file": "tests/test_slow.py",
            "baseline_ms": 61_300
        })
    );
    assert_eq!(
        (
            result.status,
            result.score.as_ref().map(|score| score.value)
        ),
        (CheckStatus::Pass, Some(1.0))
    );
}

#[test]
fn scores_combine_weighted_by_the_mutants_they_score_and_counts_add_up() {
    let check = support::check(ID);
    let python = language(
        check,
        &[("score", 0.5), ("killed", 1.0), ("survived", 1.0)],
        2,
    );
    let go = language(
        check,
        &[("score", 1.0), ("killed", 6.0), ("survived", 0.0)],
        6,
    );
    let results = [python, go];
    assert_eq!(combined(check, "score", &results), Some(7.0 / 8.0));
    assert_eq!(combined(check, "killed", &results), Some(7.0));
    let scores: Vec<cupel_core::Score> = [(0.5, 2), (1.0, 6)]
        .into_iter()
        .map(|(value, n)| support::score(value, "killed_ratio", n))
        .collect();
    assert_eq!(
        check.combine(&scores).map(|score| score.value),
        Some(7.0 / 8.0)
    );
}

/// A scope with no mutant scores 1 in every language (nothing survived), and so combined: a
/// change that touches no mutable code meets any score floor.
#[test]
fn languages_that_scored_no_mutant_combine_to_a_score_of_one() {
    let check = support::check(ID);
    let none = || {
        language(
            check,
            &[("score", 1.0), ("killed", 0.0), ("survived", 0.0)],
            0,
        )
    };
    let results = [none(), none()];
    assert_eq!(combined(check, "score", &results), Some(1.0));
    assert_eq!(combined(check, "score", &results[..1]), Some(1.0));
    let scores = [
        support::score(1.0, "killed_ratio", 0),
        support::score(1.0, "killed_ratio", 0),
    ];
    let score = check.combine(&scores).unwrap();
    assert_eq!((score.value, score.n), (1.0, 0));
}
