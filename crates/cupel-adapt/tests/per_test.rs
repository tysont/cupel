// ABOUTME: The `PerTestMap` producers run for real on the per-test fixtures (ADR-077): two tests covering disjoint
// ABOUTME: functions give two disjoint maps; skipped with a reason when a producer's tool is not installed.
use cupel_core::{
    Availability, Cancellation, FactKind, PerTestMap, ProducerRegistry, RegisteredProducer, Scope,
};
use support::{lang, produce_ctx, registry};

mod support;

fn producer<'a>(
    registry: &'a ProducerRegistry,
    id: &str,
    lang: &str,
) -> &'a dyn RegisteredProducer {
    registry
        .producers_for(FactKind::PerTestMap, &self::lang(lang))
        .find(|producer| producer.id().as_str() == id)
        .unwrap()
}

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
    let producer = producer(&registry, id, lang);
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

/// `id` measures two tests on its fixture, the one covering `add` and the one covering
/// `scale`, each alone, with the runner's IDs.
fn assert_disjoint(
    id: &str,
    lang: &str,
    runner: &str,
    file: &str,
    add: (u32, u32),
    scale: (u32, u32),
    tests: [&str; 2],
) {
    let Some(map) = measured(id, lang) else {
        return;
    };
    let ids: Vec<&str> = map.tests.iter().map(|test| test.id.as_str()).collect();
    assert_eq!((map.runner.as_str(), ids), (runner, tests.to_vec()));
    assert_eq!(covering(&map, file, add.0, add.1), [tests[0]]);
    assert_eq!(covering(&map, file, scale.0, scale.1), [tests[1]]);
}

#[test]
fn llvm_cov_measures_each_rust_test_alone() {
    assert_disjoint(
        "tool.llvm-cov-per-test",
        "rust",
        "tool.cargo-nextest",
        "src/lib.rs",
        (4, 6),
        (9, 11),
        ["calc tests::adds", "calc tests::scales"],
    );
}

#[test]
fn go_cover_measures_each_go_test_alone() {
    assert_disjoint(
        "tool.go-cover-per-test",
        "go",
        "tool.go-test",
        "calc.go",
        (5, 7),
        (10, 12),
        ["example.com/calc/TestAdd", "example.com/calc/TestScale"],
    );
}

#[test]
fn pytest_cov_measures_each_python_test_by_its_context() {
    assert_disjoint(
        "tool.pytest-cov-per-test",
        "python",
        "tool.pytest",
        "calc.py",
        (4, 6),
        (9, 11),
        [
            "tests/test_calc.py::test_add",
            "tests/test_calc.py::test_scale",
        ],
    );
}
