// ABOUTME: The joint producers run for real on the per-test fixtures (ADR-084, ADR-149): one run's three facts equal the
// ABOUTME: separate runs' facts, an unchanged tree is served from the cache, and a range reruns only the stale tests.
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;

use cupel_core::coverage::CoverageMap;
use cupel_core::syntax::{
    NormNode, NormTree, NormalizedSymbol, Span, SymbolDef, SymbolKind, Symbols, TestCase, TestFile,
    TestFrames, Visibility,
};
use cupel_core::{
    Availability, Cancellation, CostClass, Ctx, FactKind, Focus, JointProducer, LangId, PerTestMap,
    ProduceCtx, Produced, ProducerRegistry, Scope, SuiteMode, SuiteReuse, TestResults,
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
    /// The fixture's source file, and its functions (first and last line): `add`, `scale`, and
    /// the test of each, as the language's `Symbols` would have them.
    source: &'static str,
    functions: &'static [(&'static str, u32, u32)],
}

const RUST: Joint = Joint {
    id: "tool.llvm-cov-nextest",
    lang: "rust",
    coverage: "cargo llvm-cov --lcov --output-path {out}",
    runner: "tool.cargo-nextest",
    coverage_producer: "tool.cargo-llvm-cov",
    per_test: Some("tool.llvm-cov-per-test"),
    source: "src/lib.rs",
    functions: &[
        ("src/lib.rs", 4, 6),
        ("src/lib.rs", 9, 11),
        ("src/lib.rs", 18, 20),
        ("src/lib.rs", 23, 25),
    ],
};

const PYTHON: Joint = Joint {
    id: "tool.pytest-cov-contexts",
    lang: "python",
    coverage: "pytest --cov=. --cov-report=lcov:{out}",
    runner: "tool.pytest",
    coverage_producer: "tool.pytest-cov",
    per_test: Some("tool.pytest-cov-per-test"),
    source: "calc.py",
    functions: &[
        ("calc.py", 4, 6),
        ("calc.py", 9, 11),
        ("tests/test_calc.py", 5, 6),
        ("tests/test_calc.py", 9, 10),
    ],
};

const GO: Joint = Joint {
    id: "tool.go-test-cover",
    lang: "go",
    coverage: "go test -coverprofile={out} ./...",
    runner: "tool.go-test",
    coverage_producer: "tool.go-coverprofile",
    per_test: None,
    source: "calc.go",
    functions: &[],
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

impl Facts {
    /// Per file, the lines the coverage map instruments, or only those that ran: what every
    /// consumer reads of a coverage map (ADR-084 §4).
    fn lines(&self, ran_only: bool) -> BTreeMap<String, Vec<u32>> {
        self.coverage
            .iter()
            .flat_map(|map| &map.files)
            .map(|file| {
                let lines = file
                    .lines
                    .iter()
                    .filter(|hits| !ran_only || hits.hits > 0)
                    .map(|hits| hits.line)
                    .collect();
                (file.file.clone(), lines)
            })
            .collect()
    }
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

    /// [`ctx`](Self::ctx) for a run focused as `focus` says, seeing the fixture's functions as
    /// the language's `Symbols` (the joint wants them, ADR-149 §3).
    fn focused(&self, root: &Path, focus: Focus) -> ProduceCtx {
        let mut ctx = self.ctx(root);
        ctx.focus = focus;
        ctx.upstream
            .insert(Produced::new(symbols(self.lang, self.functions)).share());
        ctx
    }

    /// A range scope that changed `lines` of the fixture's source file, at `depth`.
    fn range(&self, lines: (u32, u32), depth: CostClass) -> Focus {
        Focus {
            depth,
            changed: Some(BTreeMap::from([(self.source.to_owned(), vec![lines])])),
            ..Focus::default()
        }
    }

    fn joint(&self) -> &'static dyn JointProducer {
        joint(self.id, self.lang)
    }

    /// The facts one joint run makes in `root`.
    fn jointly(&self, root: &Path) -> Facts {
        self.jointly_with(&self.ctx(root)).0
    }

    /// The facts one joint run with `ctx` makes, and how its baseline was obtained, without the
    /// run's phases, whose times differ from run to run; the joint is checked to be this one.
    fn jointly_with(&self, ctx: &ProduceCtx) -> (Facts, Option<SuiteReuse>) {
        let produced = self
            .joint()
            .produce(ctx, &Scope::default(), &mut Vec::new())
            .unwrap();
        let mut facts = Facts {
            results: Value::Null,
            coverage: None,
            per_test: None,
        };
        let mut reuse = None;
        for fact in produced {
            match fact.kind() {
                FactKind::TestResults => {
                    let results = fact.downcast::<TestResults>().unwrap();
                    reuse = results.reuse.clone().map(|mut reuse| {
                        assert_eq!(
                            reuse.joint.take().as_ref().map(|id| id.as_str()),
                            Some(self.id)
                        );
                        reuse.phases.clear();
                        reuse
                    });
                    facts.results = comparable(&results);
                }
                FactKind::CoverageMap => facts.coverage = fact.downcast::<CoverageMap>(),
                FactKind::PerTestMap => {
                    facts.per_test = fact.downcast::<PerTestMap>().map(|map| map.tests);
                }
                kind => panic!("unexpected {kind}"),
            }
        }
        (facts, reuse)
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
        self.estimate_focused(root, &Focus::default())
    }

    /// The joint's estimate in `root` for a run focused as `focus` says, seeing the fixture's
    /// functions.
    fn estimate_focused(&self, root: &Path, focus: &Focus) -> Option<String> {
        let ctx = self.focused(root, focus.clone());
        self.joint().estimate(&ctx.ctx, &ctx.upstream, focus).note
    }
}

/// `results` without what differs between two runs of one suite: durations, each invocation's
/// command line and output, and how the run was obtained.
fn comparable(results: &TestResults) -> Value {
    let mut value = serde_json::to_value(results).unwrap();
    for test in value["tests"].as_array_mut().unwrap() {
        test["duration_ms"] = Value::Null;
    }
    for invocation in value["invocations"].as_array_mut().unwrap() {
        invocation["command"] = Value::Null;
        invocation["output"] = Value::Null;
    }
    value["reuse"] = Value::Null;
    value
}

/// Replaces `from` with `to` in `file` under `root`, which must hold it.
fn replace_in(root: &Path, file: &str, from: &str, to: &str) {
    let path = root.join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "{from:?} in {file}");
    std::fs::write(&path, text.replace(from, to)).unwrap();
}

/// `Symbols` holding one function per `(file, first line, last line)`, named `f<line>`.
fn symbols(lang: &str, functions: &[(&str, u32, u32)]) -> Symbols {
    Symbols {
        symbols: functions
            .iter()
            .map(|(file, line, end_line)| SymbolDef {
                name: format!("f{line}"),
                namespace: String::new(),
                display: format!("f{line}"),
                kind: SymbolKind::Function,
                visibility: Visibility::Public,
                lang: LangId::from(lang),
                file: (*file).to_owned(),
                span: span(*line, *end_line),
                body: None,
                parameters: 0,
            })
            .collect(),
    }
}

fn span(line: u32, end_line: u32) -> Span {
    Span {
        line,
        col: 1,
        end_line,
        end_col: 1,
    }
}

/// How a joint's cache was reused by a run: its mode and counts.
fn reuse(mode: SuiteMode, rerun: u64, reused: u64) -> SuiteReuse {
    SuiteReuse {
        mode,
        rerun,
        reused,
        tagged: 0,
        joint: None,
        phases: Vec::new(),
    }
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

/// A full run on a fresh copy of `joint`'s fixture, then, after `edit`, a run with `ctx_after`'s
/// context; the second run's facts and reuse and, for comparison, a fresh full run on another
/// copy with the same edit. `None`, reported as skipped, without the tools.
fn after_edit(
    joint: &Joint,
    edit: &dyn Fn(&Path),
    ctx_after: &dyn Fn(&Path) -> ProduceCtx,
) -> Option<((Facts, Option<SuiteReuse>), Facts)> {
    let dir = joint.fixture()?;
    let root = dir.path();
    let (_, first) = joint.jointly_with(&joint.ctx(root));
    assert_eq!(
        first,
        Some(reuse(SuiteMode::Full, 2, 0)),
        "the first run is whole"
    );
    edit(root);
    let second = joint.jointly_with(&ctx_after(root));
    let fresh = joint.fixture()?;
    edit(fresh.path());
    let whole = joint.jointly(fresh.path());
    Some((second, whole))
}

/// Asserts the incremental run on `joint`'s fixture after `edit`, focused as `focus` says, was
/// obtained as `expected` says and made the facts a whole run of the edited tree makes: the
/// same tests with the same outcomes, the same instrumented lines, the same lines run, and the
/// same per-test lines.
fn assert_incremental(joint: &Joint, edit: &dyn Fn(&Path), focus: &Focus, expected: SuiteReuse) {
    let Some(((second, obtained), whole)) =
        after_edit(joint, edit, &|root| joint.focused(root, focus.clone()))
    else {
        return;
    };
    assert_eq!(obtained, Some(expected));
    assert_eq!(
        (
            &second.results,
            second.lines(false),
            second.lines(true),
            &second.per_test
        ),
        (
            &whole.results,
            whole.lines(false),
            whole.lines(true),
            &whole.per_test
        )
    );
}

/// `a + b` in `add` becomes `b + a`: a change inside the function the first test covers.
fn edit_add(joint: &'static Joint) -> impl Fn(&Path) {
    move |root| replace_in(root, joint.source, "a + b", "b + a")
}

#[test]
fn a_range_reruns_only_the_test_that_ran_the_changed_function() {
    for (joint, line) in [(&RUST, 5), (&PYTHON, 6)] {
        assert_incremental(
            joint,
            &edit_add(joint),
            &joint.range((line, line), CostClass::Standard),
            reuse(SuiteMode::Incremental, 1, 1),
        );
    }
}

#[test]
fn a_change_to_a_tests_own_body_reruns_that_test_alone() {
    assert_incremental(
        &RUST,
        &|root| replace_in(root, RUST.source, "add(2, 3), 5", "add(3, 2), 5"),
        &RUST.range((19, 19), CostClass::Standard),
        reuse(SuiteMode::Incremental, 1, 1),
    );
}

#[test]
fn a_tree_scope_or_the_deep_depth_runs_the_whole_suite() {
    for focus in [Focus::default(), RUST.range((5, 5), CostClass::Deep)] {
        assert_incremental(
            &RUST,
            &edit_add(&RUST),
            &focus,
            reuse(SuiteMode::Full, 2, 0),
        );
    }
}

#[test]
fn an_entry_measured_by_another_toolchain_runs_the_whole_suite() {
    let Some(((_, obtained), _)) = after_edit(
        &RUST,
        &|root| {
            edit_add(&RUST)(root);
            fake_toolchain(root, "tool.llvm-cov-nextest");
        },
        &|root| RUST.focused(root, RUST.range((5, 5), CostClass::Standard)),
    ) else {
        return;
    };
    assert_eq!(obtained, Some(reuse(SuiteMode::Full, 2, 0)));
}

/// Rewrites the suite entry of `joint` under `root`, when there is one, as measured by another
/// toolchain, keeping its record consistent (the record holds the report's hash).
fn fake_toolchain(root: &Path, joint: &str) {
    let dir = root.join(".cupel/cache/suite").join(joint);
    let Some(entry) = std::fs::read_dir(&dir)
        .ok()
        .and_then(|mut entries| entries.next())
        .map(|entry| entry.unwrap().path())
    else {
        return;
    };
    let mut report: Value =
        serde_json::from_slice(&std::fs::read(entry.join("report")).unwrap()).unwrap();
    report["toolchain"] = Value::from("rustc 0.0.0 nowhere");
    let bytes = serde_json::to_vec(&report).unwrap();
    let mut sources: Value =
        serde_json::from_slice(&std::fs::read(entry.join("sources.json")).unwrap()).unwrap();
    sources["report"] = Value::from(blake3::hash(&bytes).to_hex().to_string());
    std::fs::write(entry.join("report"), bytes).unwrap();
    std::fs::write(
        entry.join("sources.json"),
        serde_json::to_vec_pretty(&sources).unwrap(),
    )
    .unwrap();
}

#[test]
fn a_new_test_file_runs_the_whole_suite() {
    let Some(((second, obtained), whole)) = after_edit(
        &RUST,
        &|root| {
            std::fs::create_dir_all(root.join("tests")).unwrap();
            std::fs::write(
                root.join("tests/twice.rs"),
                "#[test]\nfn twice() {\n    assert_eq!(calc::add(1, 1), 2);\n}\n",
            )
            .unwrap();
        },
        &|root| RUST.focused(root, RUST.range((5, 5), CostClass::Standard)),
    ) else {
        return;
    };
    assert_eq!(
        (obtained, second.results),
        (Some(reuse(SuiteMode::Full, 3, 0)), whole.results)
    );
}

#[test]
fn an_unchanged_tree_is_served_from_the_cache_on_a_range_too() {
    let Some(dir) = RUST.fixture() else {
        return;
    };
    let root = dir.path();
    let first = RUST.jointly(root);
    let focus = RUST.range((5, 5), CostClass::Standard);
    let (second, obtained) = RUST.jointly_with(&RUST.focused(root, focus.clone()));
    assert_eq!(
        (
            obtained,
            RUST.estimate_focused(root, &focus),
            second.results
        ),
        (
            Some(reuse(SuiteMode::Cached, 0, 2)),
            Some("cached".to_owned()),
            first.results
        )
    );
}

#[test]
fn the_estimate_on_a_range_counts_the_stale_tests() {
    let Some(dir) = RUST.fixture() else {
        return;
    };
    let root = dir.path();
    RUST.jointly(root);
    edit_add(&RUST)(root);
    let range = RUST.range((5, 5), CostClass::Standard);
    assert_eq!(
        [
            RUST.estimate_focused(root, &range),
            RUST.estimate_focused(root, &RUST.range((5, 5), CostClass::Deep)),
            RUST.estimate(root),
        ],
        [
            Some("incremental: 1 stale".to_owned()),
            Some("one run".to_owned()),
            Some("one run".to_owned()),
        ]
    );
}

/// `TestFrames` naming the Rust fixture's two tests, and a `NormTree` giving each the hash
/// `hashes` says, so a run records them (ADR-149 §3).
fn structure(hashes: [&str; 2]) -> [Produced; 2] {
    let cases = [("adds", 18, 20), ("scales", 23, 25)];
    let frames = TestFrames {
        files: vec![TestFile {
            file: "src/lib.rs".to_owned(),
            framework: "rust-test".to_owned(),
            setup: Vec::new(),
            cases: cases
                .iter()
                .map(|(name, line, end_line)| TestCase {
                    name: (*name).to_owned(),
                    span: span(*line, *end_line),
                    assertions: Vec::new(),
                    mocks: Vec::new(),
                    skip: None,
                    property: None,
                    doc: None,
                })
                .collect(),
        }],
    };
    let norm = NormTree {
        symbols: cases
            .iter()
            .zip(hashes)
            .map(|((_, line, _), hash)| NormalizedSymbol {
                symbol: cupel_core::syntax::SymbolKey {
                    file: "src/lib.rs".to_owned(),
                    line: *line,
                    col: 1,
                },
                hash: hash.parse().unwrap(),
                root: NormNode {
                    label: "fn".to_owned(),
                    children: Vec::new(),
                    span: None,
                    text: None,
                },
            })
            .collect(),
    };
    [Produced::new(frames), Produced::new(norm)]
}

#[test]
fn a_test_whose_recorded_structural_hash_differs_is_rerun_though_no_function_it_ran_changed() {
    let outcomes: Vec<Option<SuiteReuse>> = ["sh_000000000000000b", "sh_000000000000000a"]
        .into_iter()
        .filter_map(|scales_now| {
            let dir = RUST.fixture()?;
            let root = dir.path();
            let structured = |hashes: [&str; 2]| {
                let mut ctx = RUST.focused(root, RUST.range((5, 5), CostClass::Standard));
                for fact in structure(hashes) {
                    ctx.upstream.insert(fact.share());
                }
                ctx
            };
            RUST.jointly_with(&structured(["sh_0000000000000001", "sh_000000000000000a"]));
            edit_add(&RUST)(root);
            Some(
                RUST.jointly_with(&structured(["sh_0000000000000001", scales_now]))
                    .1,
            )
        })
        .collect();
    // With both tests stale nothing is kept, so the run is a whole one (ADR-149 §2).
    if !outcomes.is_empty() {
        assert_eq!(
            outcomes,
            [
                Some(reuse(SuiteMode::Full, 2, 0)),
                Some(reuse(SuiteMode::Incremental, 1, 1))
            ]
        );
    }
}
