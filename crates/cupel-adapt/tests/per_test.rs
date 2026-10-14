// ABOUTME: The `PerTestMap` producers run for real on the per-test fixtures (ADR-077): two tests covering disjoint
// ABOUTME: functions give two disjoint maps; skipped with a reason when a producer's tool is not installed.
use cupel_core::{Availability, Cancellation, FactKind, PerTestMap, Scope};
use support::{lang, produce_ctx, producer, registry};

mod support;

#[test]
fn per_test_producers_register_for_every_language_but_the_ecma_family() {
    let registry = registry();
    let ids = ["rust", "go", "python", "javascript", "typescript"].map(|lang| {
        registry
            .producers_for(FactKind::PerTestMap, &self::lang(lang))
            .map(|producer| producer.id().to_string())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        ids,
        [
            vec!["tool.llvm-cov-per-test".to_owned()],
            vec!["tool.go-cover-per-test".to_owned()],
            vec!["tool.pytest-cov-per-test".to_owned()],
            vec![],
            vec![],
        ]
    );
}

/// The map `id` measures on a copy of `per-test/<lang>`; `None` (reported as skipped) when its
/// tool is not installed.
fn measured(id: &str, lang: &str) -> Option<PerTestMap> {
    let dir = tempfile::tempdir().unwrap();
    cupel_fixtures::copy_repo(&format!("per-test/{lang}"), dir.path());
    let registry = registry();
    let producer = producer(&registry, FactKind::PerTestMap, id, lang);
    let ctx = produce_ctx(dir.path(), lang, Cancellation::new());
    if let Availability::MissingTool { name, .. } = producer.availability(&ctx.ctx) {
        eprintln!("SKIPPED: {name} not installed; {id} was not run");
        return None;
    }
    let fact = producer
        .produce(&ctx, &Scope::default(), &mut Vec::new())
        .unwrap();
    Some(fact.downcast::<PerTestMap>().unwrap())
}

/// The tests `map` says execute `file` from `start` to `end`.
fn covering(map: &PerTestMap, file: &str, start: u32, end: u32) -> Vec<String> {
    map.index()
        .covering(file, start, end)
        .into_iter()
        .map(|(test, _)| test.id.clone())
        .collect()
}

/// What a per-test producer must measure on its fixture: two tests, the first covering `add`
/// and the second `scale` (inclusive line ranges of `file`), each alone, with `runner`'s IDs.
struct Disjoint {
    producer: &'static str,
    lang: &'static str,
    runner: &'static str,
    file: &'static str,
    add: (u32, u32),
    scale: (u32, u32),
    tests: [&'static str; 2],
}

fn assert_disjoint(expected: &Disjoint) {
    let Some(map) = measured(expected.producer, expected.lang) else {
        return;
    };
    let ids: Vec<&str> = map.tests.iter().map(|test| test.id.as_str()).collect();
    let (add, scale) = (expected.add, expected.scale);
    assert_eq!(
        (
            map.runner.as_str(),
            ids,
            covering(&map, expected.file, add.0, add.1),
            covering(&map, expected.file, scale.0, scale.1)
        ),
        (
            expected.runner,
            expected.tests.to_vec(),
            vec![expected.tests[0].to_owned()],
            vec![expected.tests[1].to_owned()]
        )
    );
}

const LLVM_COV: Disjoint = Disjoint {
    producer: "tool.llvm-cov-per-test",
    lang: "rust",
    runner: "tool.cargo-nextest",
    file: "src/lib.rs",
    add: (4, 6),
    scale: (9, 11),
    tests: ["calc tests::adds", "calc tests::scales"],
};

const GO_COVER: Disjoint = Disjoint {
    producer: "tool.go-cover-per-test",
    lang: "go",
    runner: "tool.go-test",
    file: "calc.go",
    add: (5, 7),
    scale: (10, 12),
    tests: ["example.com/calc/TestAdd", "example.com/calc/TestScale"],
};

const PYTEST_COV: Disjoint = Disjoint {
    producer: "tool.pytest-cov-per-test",
    lang: "python",
    runner: "tool.pytest",
    file: "calc.py",
    add: (4, 6),
    scale: (9, 11),
    tests: [
        "tests/test_calc.py::test_add",
        "tests/test_calc.py::test_scale",
    ],
};

#[test]
fn llvm_cov_measures_each_rust_test_alone() {
    assert_disjoint(&LLVM_COV);
}

#[test]
fn go_cover_measures_each_go_test_alone() {
    assert_disjoint(&GO_COVER);
}

#[test]
fn pytest_cov_measures_each_python_test_by_its_context() {
    assert_disjoint(&PYTEST_COV);
}
