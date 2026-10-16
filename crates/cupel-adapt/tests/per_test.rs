// ABOUTME: The `PerTestMap` producers run for real on the per-test fixtures (ADR-077, ADR-083): two tests covering disjoint
// ABOUTME: functions give two disjoint maps, and the cache keeps every test no change touches; skipped without the tool.
use std::path::Path;

use cupel_core::{
    Availability, Cancellation, CostClass, Ctx, Estimate, FactKind, PerTestMap, Scope, Upstream,
};
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
fn measured(id: &'static str, lang: &'static str) -> Option<PerTestMap> {
    let fixture = Fixture::new(id, lang)?;
    Some(fixture.produce())
}

/// A copy of `per-test/<lang>` and the producer `id` that measures it.
struct Fixture {
    dir: tempfile::TempDir,
    id: &'static str,
    lang: &'static str,
}

impl Fixture {
    /// `None`, reported as skipped, when the producer's tool is not installed.
    fn new(id: &'static str, lang: &'static str) -> Option<Self> {
        let dir = tempfile::tempdir().unwrap();
        cupel_fixtures::copy_repo(&format!("per-test/{lang}"), dir.path());
        let fixture = Self { dir, id, lang };
        let registry = registry();
        let producer = producer(&registry, FactKind::PerTestMap, id, lang);
        let ctx = produce_ctx(fixture.root(), lang, Cancellation::new());
        if let Availability::MissingTool { name, .. } = producer.availability(&ctx.ctx) {
            eprintln!("SKIPPED: {name} not installed; {id} was not run");
            return None;
        }
        Some(fixture)
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    /// The map the producer makes in `root` now.
    fn produce_in(&self, root: &Path) -> PerTestMap {
        let registry = registry();
        let producer = producer(&registry, FactKind::PerTestMap, self.id, self.lang);
        let ctx = produce_ctx(root, self.lang, Cancellation::new());
        let fact = producer
            .produce(&ctx, &Scope::default(), &mut Vec::new())
            .unwrap();
        fact.downcast::<PerTestMap>().unwrap()
    }

    fn produce(&self) -> PerTestMap {
        self.produce_in(self.root())
    }

    /// What the producer estimates producing here would cost now.
    fn estimate(&self) -> Estimate {
        let registry = registry();
        let producer = producer(&registry, FactKind::PerTestMap, self.id, self.lang);
        producer.estimate(&Ctx::new(self.root().to_path_buf()), &Upstream::default())
    }

    /// Replaces `from` with `to` in `file`, which must hold it.
    fn edit(&self, file: &str, from: &str, to: &str) {
        let path = self.root().join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{from:?} in {file}");
        std::fs::write(&path, text.replace(from, to)).unwrap();
    }

    fn write(&self, file: &str, text: &str) {
        let path = self.root().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

/// How many tests a production measured and reused, and the IDs of the map.
fn counted(map: &PerTestMap) -> (u64, u64, Vec<String>) {
    let ids = map.tests.iter().map(|test| test.id.clone()).collect();
    (map.measured, map.reused, ids)
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

const PY_ADD: &str = "tests/test_calc.py::test_add";
const PY_SCALE: &str = "tests/test_calc.py::test_scale";
const PY_TWICE: &str = "tests/test_twice.py::test_twice";

/// The Python fixture with a third test, in a file of its own, measured once.
fn python_with_a_third_test() -> Option<Fixture> {
    let fixture = Fixture::new("tool.pytest-cov-per-test", "python")?;
    fixture.write(
        "tests/test_twice.py",
        "from calc import add\n\n\ndef test_twice():\n    assert add(1, 1) == 2\n",
    );
    let first = fixture.produce();
    assert_eq!(first.measured, 3);
    Some(fixture)
}

fn ids(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn a_cached_map_is_reused_when_nothing_changed() {
    let Some(fixture) = python_with_a_third_test() else {
        return;
    };
    let second = fixture.produce();
    assert_eq!(counted(&second), (0, 3, ids(&[PY_ADD, PY_SCALE, PY_TWICE])));
    assert!(second.cached());
}

#[test]
fn editing_a_test_file_measures_only_its_test_again() {
    let Some(fixture) = python_with_a_third_test() else {
        return;
    };
    fixture.edit("tests/test_twice.py", "add(1, 1) == 2", "add(2, 2) == 4");
    assert_eq!(
        counted(&fixture.produce()),
        (1, 2, ids(&[PY_ADD, PY_SCALE, PY_TWICE]))
    );
}

#[test]
fn editing_a_source_file_measures_only_the_tests_that_cover_it_again() {
    let Some(fixture) = python_with_a_third_test() else {
        return;
    };
    fixture.write("scaling.py", "def double(x):\n    return 2 * x\n");
    fixture.write(
        "tests/test_scaling.py",
        "from scaling import double\n\n\ndef test_double():\n    assert double(2) == 4\n",
    );
    assert_eq!(fixture.produce().measured, 1);
    fixture.edit("scaling.py", "2 * x", "x * 2");
    let map = fixture.produce();
    assert_eq!((map.measured, map.reused), (1, 3));
}

#[test]
fn a_new_test_is_measured_and_a_deleted_one_dropped() {
    let Some(fixture) = python_with_a_third_test() else {
        return;
    };
    std::fs::remove_file(fixture.root().join("tests/test_twice.py")).unwrap();
    fixture.write(
        "tests/test_thrice.py",
        "from calc import scale\n\n\ndef test_thrice():\n    assert scale(1, 3) == 3\n",
    );
    assert_eq!(
        counted(&fixture.produce()),
        (
            1,
            2,
            ids(&[PY_ADD, PY_SCALE, "tests/test_thrice.py::test_thrice"])
        )
    );
}

#[test]
fn a_cache_made_on_another_machine_is_reused_through_the_tree_it_sits_in() {
    let Some(fixture) = python_with_a_third_test() else {
        return;
    };
    let elsewhere = tempfile::tempdir().unwrap();
    copy_tree(fixture.root(), elsewhere.path());
    let map = fixture.produce_in(elsewhere.path());
    assert_eq!(counted(&map), (0, 3, ids(&[PY_ADD, PY_SCALE, PY_TWICE])));
    assert_eq!(
        covering(&map, "calc.py", 4, 6),
        ids(&[PY_ADD, PY_TWICE]),
        "files are relative to the tree, not where the cache was made"
    );
}

#[test]
fn a_rust_refresh_runs_only_the_new_test_and_keeps_those_no_change_touched() {
    let Some(fixture) = Fixture::new("tool.llvm-cov-per-test", "rust") else {
        return;
    };
    assert_eq!(fixture.produce().measured, 2);
    fixture.write(
        "tests/extra.rs",
        "#[test]\nfn scales_by_one() {\n    assert_eq!(calc::scale(4, 1), 4);\n}\n",
    );
    let map = fixture.produce();
    assert_eq!(
        counted(&map),
        (
            1,
            2,
            ids(&[
                "calc tests::adds",
                "calc tests::scales",
                "calc::extra scales_by_one"
            ])
        )
    );
    assert_eq!(
        covering(&map, "src/lib.rs", 9, 11),
        ids(&["calc tests::scales", "calc::extra scales_by_one"])
    );
}

#[test]
fn a_go_test_owns_its_packages_test_files_so_editing_one_measures_its_tests_again() {
    let Some(fixture) = Fixture::new("tool.go-cover-per-test", "go") else {
        return;
    };
    fixture.write(
        "more/more.go",
        "package more\n\nfunc One() int {\n\treturn 1\n}\n",
    );
    fixture.write(
        "more/more_test.go",
        "package more\n\nimport \"testing\"\n\nfunc TestOne(t *testing.T) {\n\tif One() != 1 {\n\t\tt.Fatal(\"one\")\n\t}\n}\n",
    );
    assert_eq!(fixture.produce().measured, 3);
    fixture.edit("more/more_test.go", "One() != 1", "One() == 0");
    assert_eq!(
        counted(&fixture.produce()),
        (
            1,
            2,
            ids(&[
                "example.com/calc/TestAdd",
                "example.com/calc/TestScale",
                "example.com/calc/more/TestOne"
            ])
        )
    );
}

/// Copies every file under `from` to `to`, hidden ones (the cache) included.
fn copy_tree(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn a_producer_estimates_deep_without_a_cache_and_standard_to_reuse_or_refresh_one() {
    let Some(fixture) = Fixture::new("tool.pytest-cov-per-test", "python") else {
        return;
    };
    let mut seen = vec![fixture.estimate()];
    fixture.produce();
    seen.push(fixture.estimate());
    // Staleness is by file: both tests cover `calc.py`.
    fixture.edit("calc.py", "a + b", "b + a");
    seen.push(fixture.estimate());
    let estimate = |cost, note: Option<&str>| Estimate {
        cost,
        note: note.map(str::to_owned),
    };
    assert_eq!(
        seen,
        [
            estimate(CostClass::Deep, None),
            estimate(CostClass::Standard, Some("cached")),
            estimate(CostClass::Standard, Some("refresh 2 stale")),
        ]
    );
}
