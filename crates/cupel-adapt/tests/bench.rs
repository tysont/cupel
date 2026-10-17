// ABOUTME: The bench producers (ADR-106) on each language's perf.bench fixture with the real tools: benchmarks, units, and
// ABOUTME: sample counts, placement from Symbols, the base commit measured in an export, failures, and a clean tree.
mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use cupel_core::syntax::{Span, SymbolDef, SymbolKind, Symbols, Visibility};
use cupel_core::{
    Availability, BenchSamples, Cancellation, CostClass, FactKind, LangId, ProduceCtx, Produced,
    RunStatus, Scope,
};
use serde_json::{Value, json};

/// The fixture of `lang` copied into a git repository with one commit, and the context a
/// producer of `lang` sees there with `config` as its `[lang.<id>]` table.
fn fixture(lang: &str, config: Value) -> (tempfile::TempDir, ProduceCtx) {
    let dir = support::own_fixture("perf.bench", lang);
    let mut ctx = support::produce_ctx(dir.path(), lang, Cancellation::new());
    let Value::Object(config) = config else {
        panic!("config is a table")
    };
    ctx.config = config.clone();
    ctx.ctx.lang.insert(LangId::from(lang), config);
    (dir, ctx)
}

/// The producer `id` of `lang`, when its tool is installed; says which is not otherwise.
fn ready(id: &str, lang: &str, ctx: &ProduceCtx) -> bool {
    let producer = support::producer(support::shared_registry(), FactKind::BenchSamples, id, lang);
    match producer.availability(&ctx.ctx) {
        Availability::Ready => true,
        other => {
            eprintln!("SKIPPED: {id} is {other:?}; it did not measure {lang}");
            false
        }
    }
}

fn produce(id: &str, lang: &str, ctx: &ProduceCtx) -> BenchSamples {
    support::producer(support::shared_registry(), FactKind::BenchSamples, id, lang)
        .produce(ctx, &Scope::default(), &mut Vec::new())
        .unwrap()
        .downcast::<BenchSamples>()
        .unwrap()
}

/// (name, unit, samples) of every benchmark.
fn measured(samples: &BenchSamples) -> Vec<(String, String, usize)> {
    samples
        .benchmarks
        .iter()
        .map(|b| (b.name.clone(), b.unit.clone(), b.samples.len()))
        .collect()
}

/// Every path under `root` but `.cupel/`, relative to it.
fn files(root: &Path) -> BTreeSet<String> {
    walkdir(root, root)
}

fn walkdir(root: &Path, dir: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        if relative == ".cupel" {
            continue;
        }
        if path.is_dir() {
            found.extend(walkdir(root, &path));
        }
        found.insert(relative);
    }
    found
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn go_benchmarks_are_measured_per_unit_and_placed_at_their_function() {
    let (dir, mut ctx) = fixture("go", json!({}));
    if !ready("tool.go-bench", "go", &ctx) {
        return;
    }
    let before = files(dir.path());
    ctx.upstream.insert(
        Produced::new(Symbols {
            symbols: vec![SymbolDef {
                name: "BenchmarkSum".to_owned(),
                namespace: String::new(),
                display: "BenchmarkSum".to_owned(),
                kind: SymbolKind::Function,
                visibility: Visibility::Public,
                lang: LangId::from("go"),
                file: "sum_test.go".to_owned(),
                span: Span {
                    line: 5,
                    col: 1,
                    end_line: 10,
                    end_col: 2,
                },
                body: None,
                parameters: 1,
            }],
        })
        .share(),
    );
    let samples = produce("tool.go-bench", "go", &ctx);
    let name = "example.com/units.BenchmarkSum".to_owned();
    assert_eq!(
        measured(&samples),
        ["ns", "bytes", "allocs"].map(|unit| (name.clone(), unit.to_owned(), 5))
    );
    assert_eq!(samples.samples, 5);
    assert_eq!(samples.runs[0].status, RunStatus::Ran);
    let placed = &samples.benchmarks[0];
    assert_eq!(
        (placed.file.as_deref(), placed.line),
        (Some("sum_test.go"), Some(5))
    );
    assert_eq!(samples.benchmarks[2].samples, [0.0; 5]);
    assert_eq!(samples.base, None);
    assert_eq!(files(dir.path()), before);
}

#[test]
fn under_a_range_the_base_commit_is_measured_in_an_export() {
    let (dir, mut ctx) = fixture("go", json!({}));
    if !ready("tool.go-bench", "go", &ctx) {
        return;
    }
    git(dir.path(), &["init", "--quiet"]);
    git(dir.path(), &["add", "--all"]);
    git(dir.path(), &["commit", "--quiet", "-m", "base"]);
    let base = git(dir.path(), &["rev-parse", "HEAD"]);
    let code = dir.path().join("sum.go");
    let text = fs::read_to_string(&code).unwrap();
    fs::write(
        &code,
        text.replace(
            "total := 0",
            "values = append([]int(nil), values...)\n\ttotal := 0",
        ),
    )
    .unwrap();
    ctx.focus.base = Some(base.clone());
    ctx.focus.depth = CostClass::Quick;
    let samples = produce("tool.go-bench", "go", &ctx);
    let allocations = |benchmarks: &[cupel_core::Benchmark]| -> Vec<f64> {
        benchmarks
            .iter()
            .find(|b| b.unit == "allocs")
            .map(|b| b.samples.clone())
            .unwrap_or_default()
    };
    let measured_base = samples.base.expect("a base side");
    assert_eq!(measured_base.commit, base);
    assert_eq!(measured_base.runs[0].status, RunStatus::Ran);
    assert_eq!(allocations(&measured_base.benchmarks), [0.0; 5]);
    assert_eq!(allocations(&samples.benchmarks), [1.0; 5]);
}

#[test]
fn a_base_commit_git_cannot_export_fails_the_base_side_only() {
    let (_dir, mut ctx) = fixture("go", json!({}));
    if !ready("tool.go-bench", "go", &ctx) {
        return;
    }
    ctx.focus.base = Some("0000000".to_owned());
    let samples = produce("tool.go-bench", "go", &ctx);
    let base = samples.base.unwrap();
    assert_eq!(base.runs[0].status, RunStatus::Failed);
    assert!(
        base.runs[0]
            .detail
            .as_deref()
            .unwrap()
            .contains("cannot export"),
        "{:?}",
        base.runs
    );
    assert_eq!(samples.runs[0].status, RunStatus::Ran);
}

#[test]
fn a_go_build_failure_is_a_failed_run_with_its_output() {
    let (dir, ctx) = fixture("go", json!({}));
    if !ready("tool.go-bench", "go", &ctx) {
        return;
    }
    fs::write(
        dir.path().join("broken.go"),
        "package units\n\nfunc Broken( {\n",
    )
    .unwrap();
    let samples = produce("tool.go-bench", "go", &ctx);
    assert!(samples.benchmarks.is_empty());
    assert_eq!(samples.runs[0].status, RunStatus::Failed);
    let detail = samples.runs[0].detail.as_deref().unwrap();
    assert!(detail.contains("syntax error"), "{detail}");
}

#[test]
fn criterion_benchmarks_are_read_from_a_scratch_criterion_home() {
    let (dir, ctx) = fixture("rust", json!({}));
    if !ready("tool.criterion", "rust", &ctx) {
        return;
    }
    let before = files(dir.path());
    let samples = produce("tool.criterion", "rust", &ctx);
    assert_eq!(
        measured(&samples),
        [("sum".to_owned(), "ns".to_owned(), 10)],
        "{:?}",
        samples.runs
    );
    assert!(samples.benchmarks[0].samples.iter().all(|ns| *ns > 0.0));
    assert_eq!(files(dir.path()), before);
}

#[test]
fn pytest_benchmarks_are_named_by_node_id_and_take_the_asked_rounds() {
    let (dir, ctx) = fixture("python", json!({}));
    if !ready("tool.pytest-benchmark", "python", &ctx) {
        return;
    }
    let before = files(dir.path());
    let samples = produce("tool.pytest-benchmark", "python", &ctx);
    assert_eq!(
        measured(&samples),
        [(
            "tests/test_bench.py::test_total".to_owned(),
            "ns".to_owned(),
            5
        )],
        "{:?}",
        samples.runs
    );
    assert_eq!(
        samples.benchmarks[0].file.as_deref(),
        Some("tests/test_bench.py")
    );
    assert_eq!(files(dir.path()), before);
}

#[test]
fn hyperfine_times_each_configured_command() {
    for lang in ["javascript", "typescript"] {
        let command = if lang == "javascript" {
            "node bench.js"
        } else {
            "node bench.ts"
        };
        let (dir, ctx) = fixture(lang, json!({"bench_commands": [command]}));
        if !ready("tool.hyperfine", lang, &ctx) {
            return;
        }
        let before = files(dir.path());
        let samples = produce("tool.hyperfine", lang, &ctx);
        assert_eq!(
            measured(&samples),
            [(command.to_owned(), "ns".to_owned(), 5)],
            "{lang}: {:?}",
            samples.runs
        );
        assert_eq!(files(dir.path()), before, "{lang}");
    }
}

#[test]
fn a_command_hyperfine_cannot_run_is_a_failed_run() {
    let (_dir, ctx) = fixture("javascript", json!({"bench_commands": ["node missing.js"]}));
    if !ready("tool.hyperfine", "javascript", &ctx) {
        return;
    }
    let samples = produce("tool.hyperfine", "javascript", &ctx);
    assert!(samples.benchmarks.is_empty());
    assert_eq!(samples.runs[0].status, RunStatus::Failed);
    let detail = samples.runs[0].detail.as_deref().unwrap();
    assert!(detail.contains("non-zero exit code"), "{detail}");
}
