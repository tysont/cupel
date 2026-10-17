// ABOUTME: The race detectors (ADR-105) on the real tools: Go's -race, Miri, and ThreadSanitizer on the conc.race
// ABOUTME: fixtures report exactly their planted race and undefined behavior, narrowed to the scope, writing nothing there.
mod support;

use cupel_core::{
    Availability, Cancellation, Ctx, FactKind, RaceKind, RaceReports, RunStatus, Scope,
};
use serde_json::json;

/// The reports of the detector `id` for `lang` on a copy of its conc.race fixture, over
/// `files`, having checked the run wrote nothing in the copy outside `.cupel/`; `None`
/// (reported as skipped) when the detector is not ready here.
fn detect(id: &str, lang: &str, files: &[&str]) -> Option<RaceReports> {
    let fixture = support::own_fixture("conc.race", lang);
    let root = fixture.path();
    let before = support::tree(root);
    let producer = support::producer(support::shared_registry(), FactKind::RaceReports, id, lang);
    let ctx = support::produce_ctx(root, lang, Cancellation::new());
    if let Availability::MissingTool { name, .. } = producer.availability(&ctx.ctx) {
        eprintln!("SKIPPED: {name} not installed; {id} was not run");
        return None;
    }
    let scope = Scope::of_files(files.iter().map(|file| (*file).to_owned()).collect());
    let fact = producer.produce(&ctx, &scope, &mut Vec::new()).unwrap();
    assert_eq!(support::tree(root), before, "{id} wrote in the tree");
    Some(fact.downcast::<RaceReports>().unwrap())
}

/// Each report as (kind, test, [(access, [(function, file, line)])]).
fn summary(reports: &RaceReports) -> serde_json::Value {
    let reports: Vec<serde_json::Value> = reports
        .reports
        .iter()
        .map(|report| {
            json!([
                report.kind,
                report.test,
                report
                    .stacks
                    .iter()
                    .map(|stack| {
                        let frames: Vec<serde_json::Value> = stack
                            .frames
                            .iter()
                            .map(|frame| json!([frame.function, frame.file, frame.line]))
                            .collect();
                        json!([stack.access, frames])
                    })
                    .collect::<Vec<_>>(),
            ])
        })
        .collect();
    json!(reports)
}

#[test]
fn go_race_reports_the_write_and_read_of_the_fixture_and_nothing_outside_the_scope() {
    let Some(reports) = detect("tool.go-race", "go", &["tally.go"]) else {
        return;
    };
    assert_eq!(reports.runs.len(), 1);
    assert_eq!(reports.runs[0].status, RunStatus::Ran, "{:?}", reports.runs);
    assert_eq!(
        summary(&reports),
        json!([[
            "data-race",
            "TestPublish",
            [
                [
                    "write",
                    [["example.com/tally.Publish.func1", "tally.go", 10]]
                ],
                [
                    "read",
                    [
                        ["example.com/tally.Publish", "tally.go", 13],
                        ["example.com/tally.TestPublish", "tally_test.go", 6],
                    ]
                ],
            ],
        ]])
    );
    assert_eq!(reports.reports[0].message, "data race");
    let elsewhere = detect("tool.go-race", "go", &["other.go"]).unwrap();
    assert_eq!(elsewhere.reports, []);
}

#[test]
fn miri_reports_the_race_and_the_out_of_bounds_read_of_the_fixture() {
    let Some(reports) = detect("tool.miri", "rust", &["src/lib.rs"]) else {
        return;
    };
    assert_eq!(reports.runs[0].status, RunStatus::Ran, "{:?}", reports.runs);
    assert_eq!(
        summary(&reports),
        json!([
            [
                "data-race",
                "publishes",
                [
                    [
                        "non-atomic write",
                        [["tally::publish::{closure#0}", "src/lib.rs", 9]]
                    ],
                    ["non-atomic read", [[null, "src/lib.rs", 10]]],
                ]
            ],
            [
                "undefined-behavior",
                "reads_the_last_value",
                [[
                    null,
                    [
                        ["tally::last", "src/lib.rs", 18],
                        ["reads_the_last_value", "tests/ub.rs", 3],
                        ["reads_the_last_value::{closure#0}", "tests/ub.rs", 2],
                    ]
                ],]
            ],
        ])
    );
}

#[test]
fn thread_sanitizer_reports_the_race_of_the_fixture_where_its_runtime_runs() {
    let Some(reports) = detect("tool.tsan", "rust", &["src/lib.rs"]) else {
        return;
    };
    let run = &reports.runs[0];
    if let Some(detail) = run
        .detail
        .as_deref()
        .filter(|detail| detail.contains("signal"))
    {
        eprintln!("SKIPPED: tsan's runtime crashes the test binaries here ({detail})");
        return;
    }
    assert_eq!(run.status, RunStatus::Ran, "{:?}", reports.runs);
    let races: Vec<_> = reports
        .reports
        .iter()
        .filter(|report| report.kind == RaceKind::DataRace)
        .collect();
    assert_eq!(races.len(), 1, "{:?}", reports.reports);
    let lines: Vec<(String, u32)> = races[0]
        .stacks
        .iter()
        .filter_map(|stack| stack.frames.first())
        .map(|frame| (frame.file.clone(), frame.line))
        .collect();
    assert_eq!(
        lines,
        [("src/lib.rs".to_owned(), 9), ("src/lib.rs".to_owned(), 10)]
    );
}

#[test]
fn miri_is_preferred_for_rust_and_the_race_key_picks_thread_sanitizer() {
    let registry = support::shared_registry();
    let rust = support::lang("rust");
    let ids = |ctx: &Ctx| -> Vec<String> {
        registry
            .candidates(FactKind::RaceReports, &rust, ctx)
            .unwrap()
            .iter()
            .map(|producer| producer.id().to_string())
            .collect()
    };
    let mut ctx = Ctx::new(std::path::PathBuf::from("/repo"));
    assert_eq!(ids(&ctx), ["tool.miri", "tool.tsan"]);
    ctx.lang.insert(
        rust.id.clone(),
        serde_json::from_value(json!({"race": "tsan"})).unwrap(),
    );
    assert_eq!(ids(&ctx), ["tool.tsan"]);
    let go: Vec<String> = registry
        .producers_for(FactKind::RaceReports, &support::lang("go"))
        .map(|producer| producer.id().to_string())
        .collect();
    assert_eq!(go, ["tool.go-race"]);
    for lang in ["python", "javascript", "typescript"] {
        assert_eq!(
            registry
                .producers_for(FactKind::RaceReports, &support::lang(lang))
                .count(),
            0,
            "{lang}"
        );
    }
}
