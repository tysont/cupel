// ABOUTME: The trace validators (ADR-116): record the traces a language's tests log through Cupel's convention by
// ABOUTME: running its suite with `CUPEL_TRACE_DIR` set, then validate each against the run's spec oracle.
//! The trace validators.
//!
//! The logging convention ([`log`]): a test that exercises the implementation writes one trace
//! per behavior into the directory `CUPEL_TRACE_DIR` names, one JSON object per line per step,
//! and writes nothing when the variable is unset. A validator ([`tlc`]) needs the baseline's
//! `TestResults` for its runner, reruns the suite once with the variable pointing at a fresh
//! directory under `.cupel/cache/trace/` ([`record`]), and validates every `*.jsonl` file the
//! run left there against the specification of the run's oracle ([`spec`]).

mod log;
mod tlc;

use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::config::OracleKind;
use cupel_core::{
    Error, ProduceCtx, Result, RunOptions, Scope, TestResults, TestSelection, TraceSpec,
};

/// The variable that names the directory a test run records traces into.
pub const TRACE_DIR: &str = "CUPEL_TRACE_DIR";

/// The spec oracle of the run, when it is a specification in `lang`; why not otherwise.
pub fn spec(ctx: &ProduceCtx, lang: &str, tool: &str) -> std::result::Result<TraceSpec, String> {
    let oracle = ctx.oracle.as_ref().ok_or_else(|| {
        "the run has no oracle: define a spec oracle with `cupel oracle add NAME --kind spec \
         --spec FILE --lang tla+`"
            .to_owned()
    })?;
    let definition = &oracle.definition;
    if definition.kind != OracleKind::Spec {
        return Err(format!(
            "the run's oracle `{}` is not a specification: name a spec oracle with `--oracle`",
            oracle.name
        ));
    }
    let spec_lang = definition.lang.clone().unwrap_or_default();
    if spec_lang != lang {
        return Err(format!(
            "the run's oracle `{}` is a {spec_lang} specification, which {tool} does not read",
            oracle.name
        ));
    }
    Ok(TraceSpec {
        oracle: oracle.name.clone(),
        file: definition.spec.clone().unwrap_or_default(),
        lang: spec_lang,
    })
}

/// What a recording run left: each trace file's name and text, by name, and what went wrong.
#[derive(Debug, Default)]
pub struct Recorded {
    pub traces: Vec<(String, String)>,
    pub problem: Option<String>,
}

/// Reruns the baseline's suite with [`TRACE_DIR`] set to `.cupel/cache/trace/<runner>/`,
/// emptied first, and reads the traces it recorded there.
pub fn record(ctx: &ProduceCtx, baseline: &TestResults) -> Result<Recorded> {
    let runner = ctx
        .runners
        .get(&baseline.runner)
        .ok_or_else(|| Error::InvalidValue {
            what: "runner for the trace recording",
            value: baseline.runner.to_string(),
            expected: "a registered test runner",
        })?;
    let dir = fresh(
        ctx,
        &ctx.writer
            .cache_dir()
            .join("trace")
            .join(baseline.runner.as_str()),
    )?;
    // A seed, in the declared order, makes a runner that caches results (`go test`) run again.
    let options = RunOptions {
        env: vec![(TRACE_DIR.to_owned(), dir.to_string_lossy().into_owned())],
        seed: Some(ctx.seed),
        ..RunOptions::default()
    };
    let ran = runner.run(ctx, &TestSelection::All, &options)?;
    let mut recorded = Recorded {
        traces: traces_in(&dir),
        problem: None,
    };
    if !ran.results.is_green() {
        recorded.problem = Some(format!(
            "the test run that recorded traces did not pass ({} failing, {} incomplete); the \
             traces it recorded were validated",
            ran.results.failures().count(),
            ran.results.incomplete().count()
        ));
    } else if recorded.traces.is_empty() {
        recorded.problem = Some(format!(
            "the tests recorded no trace: a test records one by writing a `.jsonl` file, one \
             logged step per line, into the directory `{TRACE_DIR}` names"
        ));
    }
    Ok(recorded)
}

/// `dir`, emptied and created.
pub fn fresh(ctx: &ProduceCtx, dir: &Path) -> Result<PathBuf> {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|error| Error::Process {
            program: "trace".to_owned(),
            message: format!("cannot empty {}: {error}", dir.display()),
        })?;
    }
    ctx.writer.create_dir(dir)
}

/// The `*.jsonl` files under `dir`, by their path relative to it, with their text.
fn traces_in(dir: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in fs::read_dir(&next).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "jsonl") {
                files.push(path);
            }
        }
    }
    let mut traces: Vec<(String, String)> = files
        .into_iter()
        .filter_map(|path| {
            let name = path
                .strip_prefix(dir)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            Some((name, fs::read_to_string(&path).unwrap_or_default()))
        })
        .collect();
    traces.sort();
    traces
}

/// Whether a trace whose steps were logged in `files` (in the repository) touches the scope: one
/// of them is in it, or no step was placed. An empty scope holds everything.
pub fn in_scope(files: &[String], scope: &Scope) -> bool {
    scope.files.is_empty()
        || files.is_empty()
        || files.iter().any(|file| scope.files.contains(file))
}
