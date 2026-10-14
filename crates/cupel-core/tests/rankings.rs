// ABOUTME: Tests for `rankings` (ADR-062): dimensions come from manifests' per-target metrics, entries
// ABOUTME: are the highest values with deterministic ties, capped at `top`, and scope metrics never rank.
mod support;

use cupel_core::{
    CheckId, LangId, Location, Manifest, Measured, Measurement, MetricPer, MetricSpec, Target,
    rankings,
};

fn metric(name: &str, per: MetricPer) -> MetricSpec {
    MetricSpec {
        name: name.to_owned(),
        summary: format!("{name} summary"),
        per,
    }
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
    let ranked = rankings(&manifests, &[], 10);
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
    let ranked = rankings(&manifests, &values, 3);
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
