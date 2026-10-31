// ABOUTME: The coverage producers as the registry holds them: table order, availability and detection, ingest of
// ABOUTME: fresh reports, failed runs, and real runs on the coverage fixture, skipped when a tool is missing.
mod support;

use std::fs;
use std::path::Path;

use cupel_core::coverage::CoverageMap;
use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{
    Availability, Cancellation, Ctx, Event, FactKind, LangId, ProduceCtx, Produced,
    ProducerRegistry, RegisteredProducer, Scope, ToolId, Writer,
};
use cupel_fixtures::own_tmpdir::in_own_tmpdir;
use cupel_fixtures::{Capture, copy_repo, repo_path};
use serde_json::{Map, Value, json};
use support::registry;

const LANGS: [&str; 7] = [
    "rust",
    "go",
    "python",
    "javascript",
    "typescript",
    "java",
    "clojure",
];

fn producer<'a>(
    registry: &'a ProducerRegistry,
    id: &str,
    lang: &str,
) -> &'a dyn RegisteredProducer {
    registry
        .producers_for(FactKind::CoverageMap, &support::lang(lang))
        .find(|producer| producer.id().as_str() == id)
        .unwrap_or_else(|| panic!("{id} produces coverage for {lang}"))
}

/// The `[lang.<lang>]` table of the `cupel.toml` of the coverage fixture copied to `root`.
fn fixture_table(root: &Path, lang: &str) -> Map<String, Value> {
    let text = fs::read_to_string(root.join("cupel.toml")).unwrap();
    let config: Value = toml::from_str(&text).unwrap();
    config["lang"][lang].as_object().unwrap().clone()
}

/// The context of the repository at `root`, configured with the fixture's coverage command
/// when `configured`.
fn ctx(root: &Path, lang: &str, configured: bool) -> Ctx {
    let mut ctx = Ctx::new(root.to_path_buf());
    if configured {
        ctx.lang
            .insert(LangId::from(lang), fixture_table(root, lang));
    }
    ctx
}

/// What a coverage producer gets in the repository at `root`: one project root, `.`, with
/// the language's generated directories.
fn produce_ctx(root: &Path, lang: &str, configured: bool) -> ProduceCtx {
    let mut produce = ProduceCtx::new(
        ctx(root, lang, configured),
        Writer::new(root, false).unwrap().confined(),
        Cancellation::new(),
    );
    if configured {
        produce.config = fixture_table(root, lang);
    }
    let skip_dirs = match lang {
        "rust" => vec!["target"],
        "python" => vec![".venv", "venv", "__pycache__", ".tox"],
        "javascript" | "typescript" => vec!["node_modules", "dist", "coverage"],
        "java" => vec!["target", "build", ".gradle"],
        "clojure" => vec!["target", ".cpcache"],
        _ => vec!["vendor"],
    };
    produce.upstream.insert(
        Produced::new(ProjectInfo {
            roots: vec![ProjectRoot {
                path: ".".to_owned(),
                lang: LangId::from(lang),
                build_files: Vec::new(),
                members: Vec::new(),
                packages: Vec::new(),
                skip_dirs: skip_dirs.into_iter().map(str::to_owned).collect(),
            }],
        })
        .share(),
    );
    produce
}

/// A copy of the coverage fixture `repo` (named for its language, or its build tool) without its
/// committed cache.
fn fixture(repo: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo(&format!("coverage/{repo}"), dir.path());
    let _ = fs::remove_dir_all(dir.path().join(".cupel"));
    dir
}

fn produce(
    producer: &dyn RegisteredProducer,
    produce_ctx: &ProduceCtx,
) -> (cupel_core::Result<CoverageMap>, Vec<Event>) {
    let mut events = Vec::new();
    let map = producer
        .produce(produce_ctx, &Scope::default(), &mut events)
        .map(|fact| fact.downcast::<CoverageMap>().unwrap());
    (map, events)
}

/// `(file, instrumented lines, lines that ran, branches)` of `file` in `map`.
fn shape(map: &CoverageMap, file: &str) -> (String, usize, usize, Option<usize>) {
    let coverage = map
        .files
        .iter()
        .find(|coverage| coverage.file == file)
        .unwrap_or_else(|| panic!("{file} in {:?}", map.files));
    (
        coverage.file.clone(),
        coverage.lines.len(),
        coverage.lines.iter().filter(|hits| hits.hits > 0).count(),
        coverage.branches.as_ref().map(Vec::len),
    )
}

#[test]
fn coverage_producers_register_for_their_languages_in_toolchain_table_order() {
    let registry = registry();
    let ids = LANGS.map(|lang| {
        registry
            .producers_for(FactKind::CoverageMap, &support::lang(lang))
            .map(|producer| producer.id().to_string())
            .collect::<Vec<_>>()
    });
    let ecma = ["tool.vitest-coverage", "tool.jest-coverage", "tool.c8"];
    assert_eq!(
        ids,
        [
            vec!["tool.cargo-llvm-cov"],
            vec!["tool.go-coverprofile"],
            vec!["tool.pytest-cov", "tool.coverage-py"],
            ecma.to_vec(),
            ecma.to_vec(),
            vec!["tool.jacoco-maven", "tool.jacoco-gradle"],
            vec!["tool.cloverage"],
        ]
        .map(|ids| ids.into_iter().map(str::to_owned).collect::<Vec<_>>())
    );
}

#[test]
fn without_a_coverage_command_or_a_fresh_report_a_producer_is_unconfigured() {
    let registry = registry();
    let dir = fixture("go");
    let found = producer(&registry, "tool.go-coverprofile", "go").availability(&ctx(
        dir.path(),
        "go",
        false,
    ));
    assert_eq!(
        found,
        Availability::Unconfigured {
            key: "lang.go.coverage".to_owned()
        }
    );
}

#[test]
fn with_a_coverage_command_a_producer_needs_its_tool() {
    let registry = registry();
    for (id, lang, tool) in [
        ("tool.cargo-llvm-cov", "rust", "cargo-llvm-cov"),
        ("tool.go-coverprofile", "go", "go"),
        ("tool.pytest-cov", "python", "pytest"),
        ("tool.jest-coverage", "javascript", "jest"),
        ("tool.vitest-coverage", "typescript", "vitest"),
        ("tool.jacoco-maven", "java", "mvn"),
        ("tool.cloverage", "clojure", "clojure"),
    ] {
        let dir = fixture(lang);
        let found = producer(&registry, id, lang).availability(&ctx(dir.path(), lang, true));
        let expected = if cupel_core::find_program(tool).is_some() {
            Availability::Ready
        } else {
            Availability::MissingTool {
                name: ToolId::from(tool),
                install: cupel_adapt::toolchain::install(
                    &cupel_core::ProducerId::from(id),
                    tool,
                    dir.path(),
                ),
            }
        };
        assert_eq!(found, expected, "{id}");
        let probe = producer(&registry, id, lang)
            .tool(&ctx(dir.path(), lang, true))
            .unwrap();
        assert_eq!(probe.name, ToolId::from(tool), "{id}");
    }
}

#[test]
fn a_coverage_producer_is_detected_with_the_runner_it_pairs_with() {
    let registry = registry();
    let preferred = |lang: &str, ctx: &Ctx| -> String {
        registry
            .candidates(FactKind::CoverageMap, &support::lang(lang), ctx)
            .unwrap()[0]
            .id()
            .to_string()
    };
    let javascript = Ctx::new(repo_path("coverage/javascript"));
    let typescript = Ctx::new(repo_path("coverage/typescript"));
    let python = Ctx::new(repo_path("coverage/python"));
    let maven = Ctx::new(repo_path("coverage/java"));
    let gradle = Ctx::new(repo_path("coverage/gradle"));
    let mut unittest = Ctx::new(repo_path("coverage/python"));
    unittest.lang.insert(
        LangId::from("python"),
        json!({"runner": "unittest"}).as_object().unwrap().clone(),
    );
    assert_eq!(
        [
            preferred("javascript", &javascript),
            preferred("typescript", &typescript),
            preferred("python", &python),
            preferred("python", &unittest),
            preferred("java", &maven),
            preferred("java", &gradle),
        ],
        [
            "tool.jest-coverage",
            "tool.vitest-coverage",
            "tool.pytest-cov",
            "tool.coverage-py",
            "tool.jacoco-maven",
            "tool.jacoco-gradle",
        ]
    );
}

#[test]
fn a_report_the_tool_left_newer_than_every_source_is_ingested_without_running() {
    let registry = registry();
    let dir = fixture("javascript");
    let report = dir.path().join("coverage/lcov.info");
    fs::create_dir_all(report.parent().unwrap()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(
        &report,
        Capture::of("coverage/lcov", "jest").file("lcov.info"),
    )
    .unwrap();
    let jest = producer(&registry, "tool.jest-coverage", "javascript");

    let (map, events) = produce(jest, &produce_ctx(dir.path(), "javascript", false));

    assert_eq!(
        shape(&map.unwrap(), "grade.js"),
        ("grade.js".to_owned(), 19, 9, Some(14))
    );
    assert!(events.is_empty());
}

/// A committed report was made on another machine: the directory it ran in is gone, and the
/// paths it names resolve through the fixture that is here, not through that directory.
#[test]
fn a_committed_report_resolves_its_paths_through_the_tree_it_is_committed_in() {
    let registry = registry();
    let dir = tempfile::tempdir().unwrap();
    copy_repo("coverage/go", dir.path());
    let cache = dir
        .path()
        .join(".cupel/cache/coverage/tool.go-coverprofile");
    let entry = fs::read_dir(&cache)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let sources = entry.join("sources.json");
    let mut record: Value = serde_json::from_str(&fs::read_to_string(&sources).unwrap()).unwrap();
    record["ran_in"] = json!("/nonexistent/machine/go");
    fs::write(&sources, serde_json::to_string(&record).unwrap()).unwrap();
    let go = producer(&registry, "tool.go-coverprofile", "go");

    let (map, events) = produce(go, &produce_ctx(dir.path(), "go", true));

    assert_eq!(
        shape(&map.unwrap(), "grade.go"),
        ("grade.go".to_owned(), 28, 13, None)
    );
    assert!(events.is_empty());
}

#[test]
fn a_failing_coverage_command_reports_why_and_produces_nothing() {
    let registry = registry();
    let dir = fixture("go");
    let mut produce_ctx = produce_ctx(dir.path(), "go", true);
    produce_ctx.config.insert(
        "coverage".to_owned(),
        json!("echo the suite is red; exit 3"),
    );

    let (map, events) = produce(
        producer(&registry, "tool.go-coverprofile", "go"),
        &produce_ctx,
    );

    assert!(map.is_err());
    let [Event::Finding(finding)] = events.as_slice() else {
        panic!("one finding: {events:?}");
    };
    assert_eq!(
        (finding.check.as_str(), finding.kind.as_str()),
        ("coverage", "coverage.run-failed")
    );
    assert_eq!(finding.evidence.0["exit_code"], json!(3));
    assert_eq!(finding.evidence.0["output"], json!("the suite is red"));
    assert_eq!(finding.evidence.0["dir"], json!("."));
}

/// What a real run of a coverage producer must report on its fixture.
struct Expected {
    producer: &'static str,
    lang: &'static str,
    /// The coverage fixture it runs on, `coverage/<repo>`.
    repo: &'static str,
    tool: &'static str,
    /// `(file, instrumented lines, lines that ran, branches)` of the fixture's source file.
    source: (&'static str, usize, usize, Option<usize>),
    /// The fixture's test file.
    test: &'static str,
}

/// Runs `expected.producer` on its fixture for real, then checks that its cached report is
/// fresh until the test file changes.
fn assert_covers(expected: &Expected) {
    if cupel_core::find_program(expected.tool).is_none() {
        eprintln!(
            "SKIPPED: {} not installed; {} was not run",
            expected.tool, expected.producer
        );
        return;
    }
    let registry = registry();
    let dir = fixture(expected.repo);
    let tool = producer(&registry, expected.producer, expected.lang);

    let (map, events) = produce(tool, &produce_ctx(dir.path(), expected.lang, true));

    assert_eq!(events.len(), 0, "{}: {events:?}", expected.producer);
    let (file, lines, ran, branches) = expected.source;
    assert_eq!(
        shape(&map.unwrap(), file),
        (file.to_owned(), lines, ran, branches),
        "{}",
        expected.producer
    );
    let unconfigured = ctx(dir.path(), expected.lang, false);
    assert_eq!(
        tool.availability(&unconfigured),
        Availability::Ready,
        "{}: a fresh cached report is ingestible",
        expected.producer
    );
    let test = dir.path().join(expected.test);
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(&test, format!("{}\n", fs::read_to_string(&test).unwrap())).unwrap();
    assert!(
        matches!(
            tool.availability(&unconfigured),
            Availability::Unconfigured { .. }
        ),
        "{}: a changed test makes the report stale",
        expected.producer
    );
}

/// Every coverage producer the fixtures configure, with what it must report.
const EXPECTED: [Expected; 8] = [
    Expected {
        producer: "tool.cargo-llvm-cov",
        lang: "rust",
        repo: "rust",
        tool: "cargo-llvm-cov",
        source: ("src/lib.rs", 31, 14, None),
        test: "tests/grade.rs",
    },
    Expected {
        producer: "tool.go-coverprofile",
        lang: "go",
        repo: "go",
        tool: "go",
        source: ("grade.go", 28, 13, None),
        test: "grade_test.go",
    },
    Expected {
        producer: "tool.pytest-cov",
        lang: "python",
        repo: "python",
        tool: "pytest",
        source: ("grade.py", 21, 11, Some(14)),
        test: "tests/test_grade.py",
    },
    Expected {
        producer: "tool.jest-coverage",
        lang: "javascript",
        repo: "javascript",
        tool: "jest",
        source: ("grade.js", 19, 9, Some(14)),
        test: "grade.test.js",
    },
    Expected {
        producer: "tool.vitest-coverage",
        lang: "typescript",
        repo: "typescript",
        tool: "vitest",
        source: ("src/grade.ts", 18, 8, Some(14)),
        test: "src/grade.test.ts",
    },
    Expected {
        producer: "tool.jacoco-maven",
        lang: "java",
        repo: "java",
        tool: "mvn",
        source: ("src/main/java/grade/Grade.java", 18, 8, Some(16)),
        test: "src/test/java/grade/GradeTest.java",
    },
    Expected {
        producer: "tool.jacoco-gradle",
        lang: "java",
        repo: "gradle",
        tool: "gradle",
        source: ("src/main/java/grade/Grade.java", 18, 8, Some(16)),
        test: "src/test/java/grade/GradeTest.java",
    },
    Expected {
        producer: "tool.cloverage",
        lang: "clojure",
        repo: "clojure",
        tool: "clojure",
        source: ("src/grade/core.clj", 14, 8, None),
        test: "test/grade/core_test.clj",
    },
];

/// Jest keeps a cache per project path in the temporary directory, so the test runs in one of its
/// own.
#[test]
fn every_configured_coverage_tool_covers_its_fixture_and_caches_a_fresh_report() {
    in_own_tmpdir(
        "every_configured_coverage_tool_covers_its_fixture_and_caches_a_fresh_report",
        || {
            for expected in &EXPECTED {
                assert_covers(expected);
            }
        },
    );
}
