// ABOUTME: perf.bench over synthetic BenchSamples facts: Mann-Whitney U p-values (exact with ties, approximate past the
// ABOUTME: boundary), regressions and improvements past the threshold, the regression metric, notes, settings (ADR-106).
mod support;

use cupel_core::{
    ActionKind, BenchBase, BenchSamples, Benchmark, CheckId, CheckResult, CheckStatus, CostClass,
    FactKind, Facts, Finding, FindingId, MetricPer, ProducerId, RootRun, RunStatus, Scope,
    Severity,
};
use serde_json::{Value, json};
use support::{Shape, score};

const ID: &str = "perf.bench";
const NAME: &str = "example.com/units.BenchmarkSum";

fn benchmark(name: &str, unit: &str, samples: &[f64]) -> Benchmark {
    Benchmark {
        name: name.to_owned(),
        unit: unit.to_owned(),
        file: Some("sum_test.go".to_owned()),
        line: Some(7),
        samples: samples.to_vec(),
    }
}

fn ran() -> Vec<RootRun> {
    vec![RootRun {
        dir: ".".to_owned(),
        status: RunStatus::Ran,
        detail: None,
    }]
}

/// The samples `head` at head and `base` at the base commit, for one benchmark in `allocs`.
fn compared(base: &[f64], head: &[f64]) -> BenchSamples {
    BenchSamples {
        tool: ProducerId::from("tool.go-bench"),
        samples: 5,
        runs: ran(),
        benchmarks: vec![benchmark(NAME, "allocs", head)],
        base: Some(BenchBase {
            commit: "abc123".to_owned(),
            runs: ran(),
            benchmarks: vec![benchmark(NAME, "allocs", base)],
        }),
    }
}

fn run_with(samples: BenchSamples, config: Value) -> (CheckResult, Vec<Finding>) {
    let check = support::check(ID);
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(samples).unwrap();
    let scope = Scope::of_files(vec!["sum_test.go".to_owned()]);
    let (result, recorded) = support::recorded(check, &facts, scope, config).unwrap();
    (result, recorded.findings)
}

fn run(samples: BenchSamples) -> (CheckResult, Vec<Finding>) {
    run_with(samples, json!({}))
}

/// The values of `names` among the result's metrics, as text: they are exact counts and fractions.
fn metrics(result: &CheckResult, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .map(|name| result.metrics[*name].to_string())
        .collect()
}

/// `expected` as `metrics` spells them.
fn values(expected: &[f64]) -> Vec<String> {
    expected.iter().map(ToString::to_string).collect()
}

/// (kind, benchmark) of every finding.
fn kinds(findings: &[Finding]) -> Vec<(&str, &str)> {
    findings
        .iter()
        .map(|finding| {
            let benchmark = finding.evidence.0["benchmark"].as_str().unwrap_or("-");
            (finding.kind.as_str(), benchmark)
        })
        .collect()
}

/// The evidence's `p_value`.
fn p_value(finding: &Finding) -> f64 {
    finding.evidence.0["p_value"].as_f64().unwrap()
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-6 * expected.max(1e-3),
        "{actual} is not {expected}"
    );
}

#[test]
fn a_significant_slowdown_past_the_threshold_is_a_regression() {
    let (result, findings) = run(compared(
        &[10.0, 11.0, 12.0, 13.0, 14.0],
        &[20.0, 21.0, 22.0, 23.0, 24.0],
    ));
    assert_eq!(findings.len(), 1, "{findings:?}");
    let regression = &findings[0];
    assert_eq!(regression.kind, "perf.regression");
    assert_eq!(regression.severity, Severity::Warn);
    assert_eq!(regression.action.kind, ActionKind::Review);
    assert_eq!(
        regression.id,
        FindingId::for_subject(
            &CheckId::from(ID),
            "perf.regression",
            &format!("{NAME}|allocs")
        )
    );
    let mut evidence = regression.evidence.0.clone();
    assert_close(
        evidence.remove("p_value").unwrap().as_f64().unwrap(),
        2.0 / 252.0,
    );
    let change = evidence.remove("change").unwrap().as_f64().unwrap();
    assert_close(change, 10.0 / 12.0);
    assert_eq!(
        Value::Object(evidence),
        json!({
            "benchmark": NAME, "unit": "allocs", "base_median": 12.0, "head_median": 22.0,
            "base_samples": 5, "head_samples": 5, "significance": 0.05, "threshold": 0.05,
        })
    );
    let at = regression.location.as_ref().unwrap();
    assert_eq!((at.file.as_str(), at.line), ("sum_test.go", 7));
    assert_eq!(
        regression.repro,
        "cupel check --only perf.bench --base abc123"
    );
    assert_eq!(
        regression.title,
        format!("`{NAME}` regressed 83.3% in allocs (p = 0.0079)")
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert_close(result.metrics["regression"], 10.0 / 12.0);
    assert_eq!(
        metrics(
            &result,
            &["benchmarks", "compared", "regressed", "improved"]
        ),
        values(&[1.0, 1.0, 1.0, 0.0])
    );
    let scored = result.score.unwrap();
    assert_eq!((scored.unit.as_str(), scored.n), ("regression", 1));
    assert!(result.notes.is_empty(), "{:?}", result.notes);
}

#[test]
fn a_significant_speedup_is_an_improvement_and_no_regression() {
    let (result, findings) = run(compared(
        &[20.0, 21.0, 22.0, 23.0, 24.0],
        &[10.0, 11.0, 12.0, 13.0, 14.0],
    ));
    assert_eq!(kinds(&findings), [("perf.improvement", NAME)]);
    assert_eq!(findings[0].severity, Severity::Info);
    assert_close(
        findings[0].evidence.0["change"].as_f64().unwrap(),
        -10.0 / 22.0,
    );
    assert_eq!(metrics(&result, &["regression"]), values(&[0.0]));
    assert_eq!(metrics(&result, &["improved"]), values(&[1.0]));
    assert_eq!(result.status, CheckStatus::Pass);
}

#[test]
fn ties_get_midranks_in_the_exact_test() {
    // Every head sample above every base one, the values tied within each side.
    let (_, findings) = run(compared(&[1.0; 5], &[2.0; 5]));
    assert_close(p_value(&findings[0]), 2.0 / 252.0);
    // Ties across the sides: not significant at 0.05, so nothing is reported.
    let (result, findings) = run(compared(
        &[1.0, 2.0, 2.0, 3.0, 3.0],
        &[2.0, 3.0, 3.0, 4.0, 4.0],
    ));
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(metrics(&result, &["regression"]), values(&[0.0]));
}

#[test]
fn the_exact_distribution_stops_where_its_combinations_pass_one_hundred_thousand() {
    // C(18, 9) = 48 620 combinations: exact.
    let base: Vec<f64> = (0..9).map(f64::from).collect();
    let head: Vec<f64> = (100..109).map(f64::from).collect();
    let (_, findings) = run(compared(&base, &head));
    assert_close(p_value(&findings[0]), 4.113_533_525_298_231e-5);
    // C(20, 10) = 184 756: the normal approximation, with continuity and tie corrections.
    let base: Vec<f64> = (0..10).map(f64::from).collect();
    let head: Vec<f64> = (100..110).map(f64::from).collect();
    let (_, findings) = run(compared(&base, &head));
    assert_close(p_value(&findings[0]), 1.826_717_911_095_504_3e-4);
    let tied_base: Vec<f64> = [[1.0; 5], [2.0; 5]].concat();
    let tied_head: Vec<f64> = [[2.0; 5], [3.0; 5]].concat();
    let (_, findings) = run(compared(&tied_base, &tied_head));
    assert_close(p_value(&findings[0]), 2.357_069_903_963_188e-3);
}

#[test]
fn identical_or_overlapping_samples_are_not_significant() {
    for (base, head) in [
        ([5.0; 5], [5.0; 5]),
        (
            [10.0, 12.0, 14.0, 16.0, 18.0],
            [11.0, 13.0, 15.0, 17.0, 19.0],
        ),
    ] {
        let (result, findings) = run(compared(&base, &head));
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(metrics(&result, &["regression"]), values(&[0.0]));
        assert_eq!(result.status, CheckStatus::Pass);
    }
}

#[test]
fn a_significant_change_below_the_threshold_is_measured_but_not_reported() {
    let base = [100.0, 100.5, 101.0, 101.5, 102.0];
    let head = [103.0, 103.5, 104.0, 104.5, 105.0];
    let (result, findings) = run(compared(&base, &head));
    assert!(findings.is_empty(), "{findings:?}");
    assert_close(result.metrics["regression"], 3.0 / 101.0);
    let (_, findings) = run_with(compared(&base, &head), json!({"threshold_percent": 2}));
    assert_eq!(kinds(&findings), [("perf.regression", NAME)]);
    assert_eq!(findings[0].evidence.0["threshold"], json!(0.02));
}

#[test]
fn the_significance_is_a_setting() {
    let (_, findings) = run_with(
        compared(
            &[10.0, 11.0, 12.0, 13.0, 14.0],
            &[20.0, 21.0, 22.0, 23.0, 24.0],
        ),
        json!({"significance_percent": 0}),
    );
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn a_base_median_of_zero_counts_any_growth_as_a_full_regression() {
    let (result, findings) = run(compared(&[0.0; 5], &[3.0; 5]));
    assert_eq!(findings[0].evidence.0["change"], json!(1.0));
    assert_eq!(metrics(&result, &["regression"]), values(&[1.0]));
}

#[test]
fn without_a_base_nothing_is_compared_and_a_note_says_how_to_compare() {
    let mut samples = compared(&[1.0; 5], &[9.0; 5]);
    samples.base = None;
    let (result, findings) = run(samples);
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(
        metrics(&result, &["benchmarks", "compared", "regression"]),
        values(&[1.0, 0.0, 0.0])
    );
    assert_eq!(result.score, Some(score(0.0, "regression", 0)));
    assert_eq!(
        result.notes,
        ["no base to compare with: run under --changed or --base <commit>"]
    );
}

#[test]
fn failed_runs_and_benchmarks_on_one_side_are_notes() {
    let mut samples = compared(&[1.0; 5], &[1.0; 5]);
    samples.runs.push(RootRun {
        dir: "tools".to_owned(),
        status: RunStatus::Failed,
        detail: Some("build failed".to_owned()),
    });
    samples
        .benchmarks
        .push(benchmark("example.com/units.BenchmarkNew", "ns", &[1.0]));
    let base = samples.base.as_mut().unwrap();
    base.benchmarks
        .push(benchmark("example.com/units.BenchmarkOld", "ns", &[1.0]));
    base.runs[0].status = RunStatus::Failed;
    base.runs[0].detail = None;
    let (result, _) = run(samples);
    assert_eq!(
        result.notes,
        [
            "tool.go-bench failed in tools: build failed",
            "tool.go-bench failed in . at the base commit abc123: no detail",
            "measured at head only: example.com/units.BenchmarkNew (ns)",
            "measured at the base only: example.com/units.BenchmarkOld (ns)",
        ]
    );
    assert_eq!(metrics(&result, &["benchmarks"]), values(&[2.0]));
    assert_eq!(metrics(&result, &["compared"]), values(&[1.0]));
}

#[test]
fn the_regression_is_the_largest_language_s_and_counts_add_up() {
    let check = support::check(ID);
    let results = [
        support::language(
            check,
            &[("regression", 0.2), ("benchmarks", 3.0), ("regressed", 1.0)],
            3,
        ),
        support::language(
            check,
            &[("regression", 0.5), ("benchmarks", 2.0), ("regressed", 2.0)],
            2,
        ),
    ];
    assert_eq!(support::combined(check, "regression", &results), Some(0.5));
    assert_eq!(support::combined(check, "benchmarks", &results), Some(5.0));
    assert_eq!(support::combined(check, "regressed", &results), Some(3.0));
    assert_eq!(
        check.combine(&[score(0.2, "regression", 3), score(0.5, "regression", 2)]),
        Some(score(0.5, "regression", 5))
    );
}

#[test]
fn unknown_settings_are_refused_and_the_two_settings_are_listed() {
    let (key, valid) = support::setting_error(support::check(ID), json!({"anything": 1}));
    assert_eq!(
        (key.as_str(), valid),
        (
            "anything",
            vec![
                "threshold_percent".to_owned(),
                "significance_percent".to_owned()
            ]
        )
    );
}

#[test]
fn the_manifest_reads_bench_samples() {
    assert_eq!(
        Shape::of(support::check(ID).manifest()),
        Shape {
            needs: vec![FactKind::BenchSamples],
            wants: vec![],
            tier: 0,
            cost: CostClass::Deep,
            kinds: vec![
                ("perf.regression", ActionKind::Review),
                ("perf.improvement", ActionKind::Review),
            ],
            metrics: vec![
                ("benchmarks", MetricPer::Scope),
                ("compared", MetricPer::Scope),
                ("regression", MetricPer::Scope),
                ("regressed", MetricPer::Scope),
                ("improved", MetricPer::Scope),
            ],
            evidence: vec!["object", "object"],
        }
    );
}
