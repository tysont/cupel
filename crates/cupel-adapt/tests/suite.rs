// ABOUTME: The joint producers run for real on the per-test fixtures (ADR-084): one run's three facts equal the three
// ABOUTME: separate runs' facts, an unchanged tree is served from the cache, and a change runs the suite again.
use std::path::Path;
use std::sync::LazyLock;

use cupel_core::coverage::CoverageMap;
use cupel_core::{
    Availability, Cancellation, Ctx, FactKind, JointProducer, LangId, PerTestMap, ProduceCtx,
    ProducerRegistry, Scope, TestResults, Upstream,
};
use serde_json::Value;
use support::{lang, produce_ctx, producer, registry};

mod support;

/// One joint producer and the per-kind producers it stands in for, on one fixture.
struct Joint {
    id: &'static str,
    lang: &'static str,
    /// The coverage command the fixture configures: the table's default.
    coverage: &'static str,
    runner: &'static str,
    coverage_producer: &'static str,
    per_test: Option<&'static str>,
}

const RUST: Joint = Joint {
    id: "tool.llvm-cov-nextest",
    lang: "rust",
    coverage: "cargo llvm-cov --lcov --output-path {out}",
    runner: "tool.cargo-nextest",
    coverage_producer: "tool.cargo-llvm-cov",
    per_test: Some("tool.llvm-cov-per-test"),
};

const PYTHON: Joint = Joint {
    id: "tool.pytest-cov-contexts",
    lang: "python",
    coverage: "pytest --cov=. --cov-report=lcov:{out}",
    runner: "tool.pytest",
    coverage_producer: "tool.pytest-cov",
    per_test: Some("tool.pytest-cov-per-test"),
};

const GO: Joint = Joint {
    id: "tool.go-test-cover",
    lang: "go",
    coverage: "go test -coverprofile={out} ./...",
    runner: "tool.go-test",
    coverage_producer: "tool.go-coverprofile",
    per_test: None,
};

/// Every registered producer, as the binary registers them.
static REGISTRY: LazyLock<ProducerRegistry> = LazyLock::new(registry);

/// The registered joint producer `id` of `lang`.
fn joint(id: &str, lang_id: &str) -> &'static dyn JointProducer {
    REGISTRY
        .joints_for(&lang(lang_id))
        .find(|joint| joint.id().as_str() == id)
        .unwrap_or_else(|| panic!("{id} is registered"))
}

/// The three facts, as runs make them.
#[derive(Debug, PartialEq)]
struct Facts {
    results: Value,
    coverage: Option<CoverageMap>,
    per_test: Option<Vec<cupel_core::TestCoverage>>,
}

impl Joint {
    /// A copy of `per-test/<lang>` configuring the default coverage command; `None`, reported
    /// as skipped, when the joint's tools are not installed.
    fn fixture(&self) -> Option<tempfile::TempDir> {
        let dir = tempfile::tempdir().unwrap();
        cupel_fixtures::copy_repo(&format!("per-test/{}", self.lang), dir.path());
        let ctx = self.ctx(dir.path());
        if let Availability::MissingTool { name, .. } = self.joint().availability(&ctx.ctx) {
            eprintln!("SKIPPED: {name} not installed; {} was not run", self.id);
            return None;
        }
        Some(dir)
    }

    /// What a producer gets in `root`: its project root, and the coverage command configured.
    fn ctx(&self, root: &Path) -> ProduceCtx {
        let mut ctx = produce_ctx(root, self.lang, Cancellation::new());
        let mut table = serde_json::Map::new();
        table.insert("coverage".to_owned(), Value::from(self.coverage));
        ctx.ctx.lang.insert(LangId::from(self.lang), table.clone());
        ctx.config = table;
        ctx
    }

    fn joint(&self) -> &'static dyn JointProducer {
        joint(self.id, self.lang)
    }

    /// The facts one joint run makes in `root`.
    fn jointly(&self, root: &Path) -> Facts {
        let produced = self
            .joint()
            .produce(&self.ctx(root), &Scope::default(), &mut Vec::new())
            .unwrap();
        let mut facts = Facts {
            results: Value::Null,
            coverage: None,
            per_test: None,
        };
        for fact in produced {
            match fact.kind() {
                FactKind::TestResults => {
                    facts.results = comparable(&fact.downcast::<TestResults>().unwrap());
                }
                FactKind::CoverageMap => facts.coverage = fact.downcast::<CoverageMap>(),
                FactKind::PerTestMap => {
                    facts.per_test = fact.downcast::<PerTestMap>().map(|map| map.tests);
                }
                kind => panic!("unexpected {kind}"),
            }
        }
        facts
    }

    /// The facts the per-kind producers make in `root`, each in its own run.
    fn separately(&self, root: &Path) -> Facts {
        let ctx = self.ctx(root);
        let make = |kind, id| {
            producer(&REGISTRY, kind, id, self.lang)
                .produce(&ctx, &Scope::default(), &mut Vec::new())
                .unwrap()
        };
        let results = make(FactKind::TestResults, self.runner);
        let coverage = make(FactKind::CoverageMap, self.coverage_producer);
        let per_test = self.per_test.map(|id| {
            make(FactKind::PerTestMap, id)
                .downcast::<PerTestMap>()
                .unwrap()
        });
        Facts {
            results: comparable(&results.downcast::<TestResults>().unwrap()),
            coverage: coverage.downcast::<CoverageMap>(),
            per_test: per_test.map(|map| map.tests),
        }
    }

    fn estimate(&self, root: &Path) -> Option<String> {
        let ctx = self.ctx(root);
        self.joint().estimate(&ctx.ctx, &Upstream::default()).note
    }
}

/// `results` without what differs between two runs of one suite: durations, and each
/// invocation's command line and output.
fn comparable(results: &TestResults) -> Value {
    let mut value = serde_json::to_value(results).unwrap();
    for test in value["tests"].as_array_mut().unwrap() {
        test["duration_ms"] = Value::Null;
    }
    for invocation in value["invocations"].as_array_mut().unwrap() {
        invocation["command"] = Value::Null;
        invocation["output"] = Value::Null;
    }
    value
}

/// The facts of one joint run and of the separate runs, each on its own copy of the fixture.
fn compared(joint: &Joint) -> Option<(Facts, Facts)> {
    let together = joint.fixture()?;
    let apart = joint.fixture()?;
    Some((
        joint.jointly(together.path()),
        joint.separately(apart.path()),
    ))
}

fn assert_parity(joint: &Joint) {
    let Some((together, apart)) = compared(joint) else {
        return;
    };
    assert!(
        together
            .coverage
            .as_ref()
            .is_some_and(|map| !map.files.is_empty()),
        "{together:?}"
    );
    assert_eq!(together, apart);
}

#[test]
fn one_rust_run_makes_the_three_facts_the_separate_runs_make() {
    assert_parity(&RUST);
}

#[test]
fn one_python_run_makes_the_three_facts_the_separate_runs_make() {
    assert_parity(&PYTHON);
}

#[test]
fn one_go_run_makes_the_baseline_and_coverage_the_separate_runs_make() {
    assert_parity(&GO);
}

/// The estimate before a run, after it, and after a source edit; whether the second and third
/// productions came from the cache (their per-test maps measured nothing).
fn cache_lifecycle(joint: &Joint, file: &str, from: &str, to: &str) -> Option<Vec<String>> {
    let dir = joint.fixture()?;
    let root = dir.path();
    let mut seen = vec![joint.estimate(root).unwrap()];
    let first = joint.jointly(root);
    seen.push(joint.estimate(root).unwrap());
    let second = joint.jointly(root);
    assert_eq!(first, second, "the cached run makes what the run made");
    let path = root.join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from));
    std::fs::write(&path, text.replace(from, to)).unwrap();
    seen.push(joint.estimate(root).unwrap());
    Some(seen)
}

/// The estimates a run, its rerun, and a source edit go through.
const LIFECYCLE: [&str; 3] = ["one run", "cached", "one run"];

/// Asserts the estimates `joint` goes through when `a + b` in `file` becomes `b + a`.
fn assert_lifecycle(joint: &Joint, file: &str) {
    if let Some(seen) = cache_lifecycle(joint, file, "a + b", "b + a") {
        assert_eq!(seen, LIFECYCLE);
    }
}

#[test]
fn an_unchanged_python_tree_reuses_the_cached_baseline_and_a_source_change_runs_again() {
    assert_lifecycle(&PYTHON, "calc.py");
}

#[test]
fn an_unchanged_rust_tree_reuses_the_cached_baseline_and_a_source_change_runs_again() {
    assert_lifecycle(&RUST, "src/lib.rs");
}

#[test]
fn a_customized_coverage_command_leaves_the_joint_unconfigured() {
    let mut ctx = Ctx::new(std::env::temp_dir());
    let mut table = serde_json::Map::new();
    table.insert(
        "coverage".to_owned(),
        Value::from("pytest --cov --cov-branch --cov-report=lcov:{out}"),
    );
    ctx.lang.insert(LangId::from("python"), table);
    assert_eq!(
        joint(PYTHON.id, "python").availability(&ctx),
        Availability::Unconfigured {
            key: "lang.python.coverage".to_owned()
        }
    );
}
