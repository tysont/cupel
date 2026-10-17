// ABOUTME: `tool.analyzers` (ADR-102) on the real tools: each language's static.analyze fixture reports exactly its
// ABOUTME: planted results, results outside the scope are dropped, and unconfigured or failing analyzers are recorded.
mod support;

use std::fs;
use std::path::Path;

use cupel_core::{
    AnalyzerLevel, AnalyzerReport, AnalyzerStatus, Availability, Cancellation, FactKind,
    ProducerRegistry, Scope, find_program,
};
use cupel_fixtures::copy_repo;

const ID: &str = "tool.analyzers";

/// Every program the fixtures' analyzers run.
const PROGRAMS: [&str; 7] = [
    "cargo",
    "mypy",
    "ruff",
    "tsc",
    "eslint",
    "go",
    "staticcheck",
];

fn installed() -> bool {
    match PROGRAMS
        .iter()
        .find(|program| find_program(program).is_none())
    {
        Some(program) => {
            eprintln!("SKIPPED: {program} not installed; the analyzers were not run");
            false
        }
        None => true,
    }
}

fn registry() -> &'static ProducerRegistry {
    static REGISTRY: std::sync::OnceLock<ProducerRegistry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(support::registry)
}

/// The analyzers' report on `files` of the repository at `root` for `lang`.
fn analyze(root: &Path, lang: &str, files: &[&str]) -> AnalyzerReport {
    let producer = support::producer(registry(), FactKind::AnalyzerReport, ID, lang);
    let ctx = support::produce_ctx(root, lang, Cancellation::new());
    assert_eq!(producer.availability(&ctx.ctx), Availability::Ready);
    let scope = Scope::of_files(files.iter().map(|file| (*file).to_owned()).collect());
    producer
        .produce(&ctx, &scope, &mut Vec::new())
        .unwrap()
        .downcast::<AnalyzerReport>()
        .unwrap()
}

/// A copy of the static.analyze fixture for `lang`.
fn fixture(lang: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo(&format!("static.analyze/{lang}"), dir.path());
    dir
}

/// One result as (rule, level, file, line).
type Found = (String, AnalyzerLevel, String, u32);

/// One analyzer as (tool, status, its results).
type Analyzed = (String, AnalyzerStatus, Vec<Found>);

type Summary = Vec<Analyzed>;

/// Each analyzer as (tool, status, [(rule, level, file, line)]).
fn summary(report: &AnalyzerReport) -> Summary {
    report
        .analyzers
        .iter()
        .map(|run| {
            (
                run.tool.to_string(),
                run.status,
                run.results
                    .iter()
                    .map(|result| {
                        (
                            result.rule.clone(),
                            result.level,
                            result.location.file.clone(),
                            result.location.line,
                        )
                    })
                    .collect(),
            )
        })
        .collect()
}

fn ran(tool: &str, results: &[(&str, AnalyzerLevel, &str, u32)]) -> Analyzed {
    (
        tool.to_owned(),
        AnalyzerStatus::Ran,
        results
            .iter()
            .map(|(rule, level, file, line)| {
                ((*rule).to_owned(), *level, (*file).to_owned(), *line)
            })
            .collect(),
    )
}

#[test]
fn each_language_reports_exactly_its_fixture_s_planted_results() {
    use AnalyzerLevel::{Error, Warning};
    if !installed() {
        return;
    }
    let cases: [(&str, &[&str], Summary); 5] = [
        (
            "rust",
            &["src/lib.rs"],
            vec![ran(
                "clippy",
                &[("needless_return", Warning, "src/lib.rs", 2)],
            )],
        ),
        (
            "python",
            &["ledger.py"],
            vec![
                ran("mypy", &[("arg-type", Error, "ledger.py", 9)]),
                ran("ruff", &[("F401", Warning, "ledger.py", 1)]),
            ],
        ),
        (
            "javascript",
            &["ledger.js"],
            vec![ran(
                "eslint",
                &[
                    ("eqeqeq", Error, "ledger.js", 2),
                    ("no-unused-vars", Error, "ledger.js", 5),
                ],
            )],
        ),
        (
            "typescript",
            &["ledger.ts"],
            vec![
                ran("tsc", &[("TS2345", Error, "ledger.ts", 5)]),
                (
                    "eslint".to_owned(),
                    AnalyzerStatus::Unconfigured,
                    Vec::new(),
                ),
            ],
        ),
        (
            "go",
            &["ledger.go"],
            vec![
                ran("go-vet", &[("printf", Warning, "ledger.go", 6)]),
                ran("staticcheck", &[("SA5009", Error, "ledger.go", 6)]),
            ],
        ),
    ];
    for (lang, files, expected) in cases {
        let dir = fixture(lang);
        let report = analyze(dir.path(), lang, files);
        assert_eq!(summary(&report), expected, "{lang}");
    }
}

#[test]
fn an_unconfigured_analyzer_says_what_to_add() {
    if !installed() {
        return;
    }
    let dir = fixture("typescript");
    let report = analyze(dir.path(), "typescript", &["ledger.ts"]);
    let eslint = &report.analyzers[1];
    assert_eq!(eslint.status, AnalyzerStatus::Unconfigured);
    assert!(
        eslint
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("eslint.config.js")),
        "{eslint:?}"
    );
}

#[test]
fn results_outside_the_scope_are_dropped_though_the_project_is_analyzed() {
    if !installed() {
        return;
    }
    let dir = fixture("typescript");
    fs::write(
        dir.path().join("other.ts"),
        "export const late: string = 1;\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("tsconfig.json"),
        "{\n  \"compilerOptions\": {\"strict\": true, \"noEmit\": true},\n  \"files\": [\"ledger.ts\", \"other.ts\"]\n}\n",
    )
    .unwrap();
    let both = analyze(dir.path(), "typescript", &["ledger.ts", "other.ts"]);
    let files: Vec<&str> = both.analyzers[0]
        .results
        .iter()
        .map(|result| result.location.file.as_str())
        .collect();
    assert_eq!(files, ["ledger.ts", "other.ts"]);
    let one = analyze(dir.path(), "typescript", &["other.ts"]);
    assert_eq!(
        summary(&one)[0],
        ran("tsc", &[("TS2322", AnalyzerLevel::Error, "other.ts", 1)])
    );
}

#[test]
fn an_analyzer_that_fails_is_recorded_with_its_output_and_the_others_still_run() {
    if !installed() {
        return;
    }
    let dir = fixture("python");
    fs::write(dir.path().join("ruff.toml"), "line-length = [\n").unwrap();
    let report = analyze(dir.path(), "python", &["ledger.py"]);
    assert_eq!(report.analyzers[0].status, AnalyzerStatus::Ran);
    assert_eq!(report.analyzers[0].results.len(), 1);
    let ruff = &report.analyzers[1];
    assert_eq!(ruff.status, AnalyzerStatus::Failed);
    assert!(ruff.results.is_empty());
    assert!(
        ruff.detail
            .as_deref()
            .is_some_and(|detail| detail.contains("ruff.toml")),
        "{ruff:?}"
    );
}

#[test]
fn analyzing_writes_nothing_in_the_tree_but_cargo_s_target() {
    if !installed() {
        return;
    }
    for (lang, file) in [
        ("python", "ledger.py"),
        ("typescript", "ledger.ts"),
        ("javascript", "ledger.js"),
        ("go", "ledger.go"),
    ] {
        let dir = fixture(lang);
        let before = listing(dir.path());
        analyze(dir.path(), lang, &[file]);
        let after: Vec<String> = listing(dir.path())
            .into_iter()
            .filter(|path| !path.starts_with(".cupel"))
            .collect();
        assert_eq!(after, before, "{lang}");
    }
}

/// Every path under `root`, relative to it, sorted.
fn listing(root: &Path) -> Vec<String> {
    let mut paths: Vec<String> = ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(false)
        .build()
        .flatten()
        .filter_map(|entry| {
            let relative = entry.path().strip_prefix(root).ok()?;
            let text = relative.to_string_lossy().into_owned();
            (!text.is_empty()).then_some(text)
        })
        .collect();
    paths.sort();
    paths
}
