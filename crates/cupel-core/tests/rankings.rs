// ABOUTME: Tests for `rankings` (ADR-062, ADR-132): dimensions from manifests' per-target metrics, worst values
// ABOUTME: first with deterministic ties, capped at `top`; and the protect-first targets joined across them.
mod support;

use cupel_core::{
    CheckId, Finding, LangId, Location, Manifest, Measured, Measurement, MetricPer, MetricSpec,
    Protection, ProtectionMetric, Recommendation, Target, Worst, rankings,
};
use serde_json::json;

fn metric(name: &str, per: MetricPer) -> MetricSpec {
    MetricSpec {
        name: name.to_owned(),
        summary: format!("{name} summary"),
        per,
        protection: None,
    }
}

fn protection(name: &str, protection: ProtectionMetric) -> MetricSpec {
    MetricSpec {
        protection: Some(protection),
        ..metric(name, MetricPer::Symbol)
    }
}

/// A structural hash for the symbol `display`, distinct per display name.
fn hash(display: &str) -> String {
    let digits: String = display.bytes().map(|b| format!("{b:02x}")).collect();
    format!("sh_{digits:0>16}")
}

fn manifest(id: &str, metrics: Vec<MetricSpec>) -> Manifest {
    Manifest {
        metrics,
        ..support::manifest(id, &[], &[])
    }
}

fn measured(check: &str, lang: &str, metric: &str, display: &str, value: f64) -> Measured {
    Measured {
        check: CheckId::from(check),
        lang: LangId::from(lang),
        measurement: Measurement {
            metric: metric.to_owned(),
            value,
            target: Target {
                display: display.to_owned(),
                symbol: None,
                location: Some(Location {
                    file: format!("{display}.rs"),
                    line: 1,
                    col: None,
                    end_line: 1,
                    end_col: None,
                }),
            },
        },
    }
}

#[test]
fn dimensions_are_every_per_target_metric_in_manifest_order() {
    let manifests = [
        manifest(
            "static.complexity",
            vec![
                metric("cyclomatic_max", MetricPer::Scope),
                metric("cyclomatic", MetricPer::Symbol),
                metric("cognitive", MetricPer::Symbol),
            ],
        ),
        manifest("static.deps", vec![metric("coupling", MetricPer::Module)]),
        manifest(
            "static.clones",
            vec![metric("cluster_size", MetricPer::Group)],
        ),
    ];
    let ranked = rankings(&manifests, &[], &[], 10);
    let dimensions: Vec<(String, &str, MetricPer)> = ranked
        .dimensions
        .iter()
        .map(|d| (d.check.to_string(), d.metric.as_str(), d.per))
        .collect();
    assert_eq!(
        dimensions,
        [
            (
                "static.complexity".to_owned(),
                "cyclomatic",
                MetricPer::Symbol
            ),
            (
                "static.complexity".to_owned(),
                "cognitive",
                MetricPer::Symbol
            ),
            ("static.deps".to_owned(), "coupling", MetricPer::Module),
            ("static.clones".to_owned(), "cluster_size", MetricPer::Group),
        ]
    );
    assert!(
        ranked
            .dimensions
            .iter()
            .all(|d| d.entries.is_empty() && d.measured == 0)
    );
    assert_eq!(ranked.dimensions[0].summary, "cyclomatic summary");
    assert_eq!(ranked.dimensions[0].worst, Worst::Highest);
    assert_eq!(ranked.top, 10);
}

#[test]
fn entries_are_the_highest_values_with_ties_broken_by_name_and_capped_at_top() {
    let manifests = [manifest(
        "static.complexity",
        vec![metric("cyclomatic", MetricPer::Symbol)],
    )];
    let values = [
        measured("static.complexity", "rust", "cyclomatic", "b", 4.0),
        measured("static.complexity", "go", "cyclomatic", "a", 4.0),
        measured("static.complexity", "rust", "cyclomatic", "c", 9.0),
        measured("static.complexity", "rust", "cyclomatic", "d", 1.0),
        measured("static.complexity", "rust", "cognitive", "e", 99.0),
        measured("static.other", "rust", "cyclomatic", "f", 99.0),
    ];
    let ranked = rankings(&manifests, &values, &[], 3);
    let dimension = &ranked.dimensions[0];
    assert_eq!(dimension.measured, 4);
    let entries: Vec<(u64, f64, &str, String)> = dimension
        .entries
        .iter()
        .map(|e| {
            (
                e.rank,
                e.value,
                e.target.display.as_str(),
                e.lang.to_string(),
            )
        })
        .collect();
    assert_eq!(
        entries,
        [
            (1, 9.0, "c", "rust".to_owned()),
            (2, 4.0, "a", "go".to_owned()),
            (3, 4.0, "b", "rust".to_owned()),
        ]
    );
    assert_eq!(ranked.top, 3);
}

/// A measurement of the symbol `display` in Rust, with its structural hash.
fn of_symbol(check: &str, metric: &str, display: &str, value: f64) -> Measured {
    let mut measured = measured(check, "rust", metric, display, value);
    measured.measurement.target.symbol = Some(hash(display).parse().unwrap());
    measured
}

/// A finding about the symbol `display` whose action is `action`.
fn finding(id: &str, check: &str, display: &str, action: &str) -> Finding {
    serde_json::from_value(json!({
        "id": id, "check": check, "kind": format!("{check}.kind"),
        "severity": "warn", "confidence": 1.0, "tier": 0,
        "symbol": {"display": display, "hash": hash(display), "lang": "rust"},
        "location": {"file": format!("{display}.rs"), "line": 7, "col": null, "end_line": 7, "end_col": null},
        "title": "t", "detail": null, "evidence": {},
        "action": {"kind": action, "priority": 1, "summary": format!("act on {display}"), "hints": {}},
        "repro": "cupel check", "suppress": "cupel suppress", "new_since_baseline": false,
        "suppressed": false
    }))
    .unwrap()
}

/// The checks of a standard survey: complexity, CRAP, and the two protection checks.
fn standard() -> [Manifest; 4] {
    [
        manifest(
            "static.complexity",
            vec![metric("cyclomatic", MetricPer::Symbol)],
        ),
        manifest("static.crap", vec![metric("crap", MetricPer::Symbol)]),
        manifest(
            "tests.coverage",
            vec![protection("line_coverage", ProtectionMetric::Coverage)],
        ),
        manifest(
            "tests.mutation",
            vec![
                protection("mutation_score", ProtectionMetric::MutationScore),
                protection("tests", ProtectionMetric::Tests),
            ],
        ),
    ]
}

/// Each target as (rank, display, recommendation).
fn order(ranked: &cupel_core::Rankings) -> Vec<(u64, &str, Recommendation)> {
    ranked
        .targets
        .iter()
        .map(|t| (t.rank, t.target.display.as_str(), t.recommend))
        .collect()
}

#[test]
fn a_protection_dimension_ranks_its_lowest_values_first() {
    let values = [
        of_symbol("tests.mutation", "mutation_score", "a", 0.9),
        of_symbol("tests.mutation", "mutation_score", "b", 0.2),
        of_symbol("tests.mutation", "mutation_score", "c", 1.0),
    ];
    let ranked = rankings(&standard(), &values, &[], 10);
    let score = ranked
        .dimensions
        .iter()
        .find(|d| d.metric == "mutation_score")
        .unwrap();
    let entries: Vec<(u64, &str)> = score
        .entries
        .iter()
        .map(|e| (e.rank, e.target.display.as_str()))
        .collect();
    assert_eq!(
        (score.worst, entries),
        (Worst::Lowest, vec![(1, "b"), (2, "a"), (3, "c")])
    );
}

#[test]
fn an_unprotected_complex_symbol_outranks_a_protected_worse_one() {
    let values = [
        of_symbol("static.complexity", "cyclomatic", "tested", 30.0),
        of_symbol("tests.mutation", "mutation_score", "tested", 1.0),
        of_symbol("tests.coverage", "line_coverage", "tested", 1.0),
        of_symbol("static.complexity", "cyclomatic", "untested", 12.0),
        of_symbol("tests.coverage", "line_coverage", "untested", 0.0),
    ];
    let findings = [
        finding(
            "fnd_000001",
            "static.complexity",
            "tested",
            "reduce_complexity",
        ),
        finding(
            "fnd_000002",
            "static.complexity",
            "untested",
            "reduce_complexity",
        ),
        finding("fnd_000003", "tests.coverage", "untested", "write_test"),
    ];
    let ranked = rankings(&standard(), &values, &findings, 10);
    assert_eq!(
        order(&ranked),
        [
            (1, "untested", Recommendation::Both),
            (2, "tested", Recommendation::Simplify),
        ]
    );
    assert_eq!(ranked.recommended, 2);
    let untested = &ranked.targets[0];
    assert_eq!(
        untested.protection,
        Protection {
            mutation_score: None,
            coverage: Some(0.0),
            tests: None,
        }
    );
    let ids: Vec<String> = untested.findings.iter().map(ToString::to_string).collect();
    assert_eq!(ids, ["fnd_000002", "fnd_000003"]);
    let values: Vec<(&str, f64, u64)> = untested
        .values
        .iter()
        .map(|v| (v.metric.as_str(), v.value, v.rank))
        .collect();
    assert_eq!(values, [("cyclomatic", 12.0, 2), ("line_coverage", 0.0, 1)]);
    assert_eq!(untested.lang, LangId::from("rust"));
    assert_eq!(
        untested.target.location.as_ref().map(|l| l.line),
        Some(1),
        "the measured symbol's span, not the finding's site"
    );
}

#[test]
fn the_mutation_score_is_the_protection_and_coverage_stands_in_where_it_is_absent() {
    let values = [
        of_symbol("tests.mutation", "mutation_score", "scored", 0.5),
        of_symbol("tests.coverage", "line_coverage", "scored", 0.1),
        of_symbol("tests.coverage", "line_coverage", "covered", 0.3),
    ];
    let findings = [
        finding(
            "fnd_000001",
            "tests.mutation",
            "scored",
            "strengthen_assertion",
        ),
        finding("fnd_000002", "tests.coverage", "covered", "write_test"),
    ];
    let ranked = rankings(&standard(), &values, &findings, 10);
    assert_eq!(
        order(&ranked),
        [
            (1, "covered", Recommendation::Protect),
            (2, "scored", Recommendation::Protect),
        ]
    );
}

#[test]
fn equal_protection_falls_back_to_the_metric() {
    let values = [
        of_symbol("static.complexity", "cyclomatic", "a", 8.0),
        of_symbol("static.crap", "crap", "a", 9.0),
        of_symbol("tests.mutation", "mutation_score", "a", 0.5),
        of_symbol("static.complexity", "cyclomatic", "b", 20.0),
        of_symbol("static.crap", "crap", "b", 4.0),
        of_symbol("tests.mutation", "mutation_score", "b", 0.5),
        of_symbol("static.complexity", "cyclomatic", "c", 9.0),
        of_symbol("static.crap", "crap", "c", 2.0),
        of_symbol("tests.mutation", "mutation_score", "c", 0.5),
    ];
    let findings = [
        finding("fnd_000001", "tests.mutation", "c", "strengthen_assertion"),
        finding("fnd_000002", "tests.mutation", "a", "strengthen_assertion"),
        finding("fnd_000003", "tests.mutation", "b", "strengthen_assertion"),
    ];
    let ranked = rankings(&standard(), &values, &findings, 10);
    // b is the most complex and a the highest CRAP, so each ranks first somewhere; c ranks
    // second at best; a and b tie and fall back to their names.
    assert_eq!(
        order(&ranked),
        [
            (1, "a", Recommendation::Protect),
            (2, "b", Recommendation::Protect),
            (3, "c", Recommendation::Protect),
        ]
    );
}

#[test]
fn targets_are_symbols_with_a_protect_or_simplify_finding_capped_at_top() {
    let values = [
        of_symbol("static.complexity", "cyclomatic", "a", 5.0),
        of_symbol("static.complexity", "cyclomatic", "b", 4.0),
        of_symbol("static.complexity", "cyclomatic", "c", 3.0),
        of_symbol("static.complexity", "cyclomatic", "quiet", 50.0),
    ];
    let mut suppressed = finding("fnd_000004", "static.clones", "c", "refactor_extract");
    suppressed.suppressed = true;
    let findings = [
        finding("fnd_000001", "static.complexity", "a", "reduce_complexity"),
        finding("fnd_000002", "static.clones", "b", "refactor_extract"),
        suppressed,
        finding("fnd_000005", "static.complexity", "quiet", "review"),
    ];
    let ranked = rankings(&standard(), &values, &findings, 1);
    assert_eq!(
        (ranked.recommended, order(&ranked)),
        (2, vec![(1, "a", Recommendation::Simplify)])
    );
}
