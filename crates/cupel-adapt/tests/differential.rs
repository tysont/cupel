// ABOUTME: The differential engine (ADR-093) on real interpreters and compilers: each language's harness against a
// ABOUTME: command oracle and against the commit it changed, minimization, skips, and the oracle's failures.
use std::fs;
use std::path::Path;
use std::process::Command;

use cupel_core::config::{ConfigFile, OracleConfig, OracleKind};
use cupel_core::{
    Answer, Availability, Cancellation, Ctx, DivergenceResults, EntryComparison, Error, FactKind,
    LangId, ProduceCtx, Produced, ProducerRegistry, RegisteredProducer, RunOracle, Scope, Writer,
};
use cupel_fixtures::copy_repo;
use cupel_fixtures::matrix::Registries;
use serde_json::{Map, Value, json};

const SEED: u64 = cupel_fixtures::matrix::SEED;

fn registry() -> &'static ProducerRegistry {
    &Registries::shared().producers
}

fn engine(lang: &str) -> &'static dyn RegisteredProducer {
    registry()
        .producers_for(
            FactKind::DivergenceResults,
            &Registries::shared().lang(lang),
        )
        .next()
        .unwrap_or_else(|| panic!("engine.differential serves {lang}"))
}

/// `kind` for `lang` as the first ready producer makes it in `ctx`, its needs made first.
fn made(kind: FactKind, lang: &str, ctx: &ProduceCtx, scope: &Scope) -> Produced {
    let producer = registry()
        .producers_for(kind, &Registries::shared().lang(lang))
        .find(|producer| producer.availability(&ctx.ctx) == Availability::Ready)
        .unwrap_or_else(|| panic!("a ready producer of {kind} for {lang}"));
    let mut ctx = ctx.clone();
    for need in producer.needs() {
        ctx.upstream.insert(made(*need, lang, &ctx, scope).share());
    }
    producer.produce(&ctx, scope, &mut Vec::new()).unwrap()
}

/// The fixture `gen.differential/<lang>` copied into `dir`: its `[lang.<lang>]` table and its
/// `quarter` oracle.
fn fixture(lang: &str, dir: &Path) -> (Map<String, Value>, RunOracle) {
    copy_repo(&format!("gen.differential/{lang}"), dir);
    let path = dir.join("cupel.toml");
    let file = ConfigFile::parse(&fs::read_to_string(&path).unwrap(), &path).unwrap();
    let table = file.lang[&LangId::from(lang)].clone();
    let oracle = RunOracle {
        name: "quarter".to_owned(),
        definition: file.oracles["quarter"].clone(),
    };
    (table, oracle)
}

/// A run's context for `lang` at `root` with the `[lang.<lang>]` table `table`, `oracle`, and
/// `--cases`.
fn run_ctx(
    lang: &str,
    root: &Path,
    table: &Map<String, Value>,
    oracle: Option<RunOracle>,
    cases: Option<u64>,
) -> ProduceCtx {
    let mut base = Ctx::new(root.to_path_buf());
    base.lang.insert(LangId::from(lang), table.clone());
    let mut ctx = ProduceCtx::new(
        base,
        Writer::new(root, false).unwrap().confined(),
        Cancellation::new(),
    );
    ctx.config.clone_from(table);
    ctx.seed = SEED;
    ctx.oracle = oracle;
    ctx.focus.cases = cases;
    ctx
}

/// What the engine makes for `lang` in `ctx` over the files its entries name; `None` (reported
/// as skipped) when the language's program is not installed.
fn compared(lang: &str, ctx: &ProduceCtx) -> Option<cupel_core::Result<DivergenceResults>> {
    if let Availability::MissingTool { name, .. } = engine(lang).availability(&ctx.ctx) {
        eprintln!("SKIPPED: {name} not installed; engine.differential did not compare {lang}");
        return None;
    }
    let files = ctx.config["differential_entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .unwrap()
                .rsplit_once(':')
                .unwrap()
                .0
                .to_owned()
        })
        .collect();
    let scope = Scope::of_files(files);
    let mut making = ctx.clone();
    making.cancel = Cancellation::new();
    let mut ctx = ctx.clone();
    for kind in engine(lang).needs() {
        ctx.upstream
            .insert(made(*kind, lang, &making, &scope).share());
    }
    Some(
        engine(lang)
            .produce(&ctx, &scope, &mut Vec::new())
            .map(|fact| fact.downcast::<DivergenceResults>().unwrap()),
    )
}

/// The one entry the engine compared for `lang` in `ctx`.
fn only_entry(lang: &str, ctx: &ProduceCtx) -> Option<EntryComparison> {
    let results = compared(lang, ctx)?.unwrap();
    assert_eq!(results.entries.len(), 1, "{results:#?}");
    results.entries.into_iter().next()
}

/// `quarter` rounds down and the oracle toward zero: after the boundaries 0 and 1 (and a
/// command oracle's two known inputs before them), input `case`, -1, diverges and is already
/// minimal; `compared` inputs were answered by both sides.
fn assert_quarter_diverges_at_minus_one(entry: &EntryComparison, case: u64, compared: u64) {
    let divergence = entry
        .divergence
        .as_ref()
        .unwrap_or_else(|| panic!("{entry:#?}"));
    assert_eq!(
        (
            &divergence.input,
            &divergence.found,
            divergence.case,
            divergence.shrinks
        ),
        (&vec![json!(-1)], &vec![json!(-1)], case, 0),
        "{entry:#?}"
    );
    assert_eq!(
        (&divergence.implementation, &divergence.oracle),
        (
            &Answer::Returned("-1".to_owned()),
            &Answer::Returned("0".to_owned())
        )
    );
    assert_eq!((entry.compared, &entry.skipped), (compared, &None));
}

/// Every file under `dir` outside `.cupel/`, relative to it, sorted.
fn files(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in fs::read_dir(&next).unwrap().flatten() {
            let path = entry.path();
            if !path.is_dir() {
                found.push(path.strip_prefix(dir).unwrap().display().to_string());
            } else if entry.file_name() != ".cupel" {
                pending.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The fixture of `lang` against its command oracle, which leaves the tree as it was.
fn assert_command_divergence(lang: &str) {
    let dir = tempfile::tempdir().unwrap();
    let (table, oracle) = fixture(lang, dir.path());
    let before = files(dir.path());
    let ctx = run_ctx(lang, dir.path(), &table, Some(oracle), None);
    if let Some(entry) = only_entry(lang, &ctx) {
        assert_quarter_diverges_at_minus_one(&entry, 4, 5);
        assert_eq!(files(dir.path()), before);
    }
}

#[test]
fn python_diverges_from_a_command_oracle_at_minus_one() {
    assert_command_divergence("python");
}

#[test]
fn javascript_diverges_from_a_command_oracle_at_minus_one() {
    assert_command_divergence("javascript");
}

#[test]
fn typescript_diverges_from_a_command_oracle_at_minus_one() {
    assert_command_divergence("typescript");
}

#[test]
fn go_diverges_from_a_command_oracle_at_minus_one() {
    assert_command_divergence("go");
}

#[test]
fn rust_diverges_from_a_command_oracle_at_minus_one() {
    assert_command_divergence("rust");
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Cupel")
        .env("GIT_AUTHOR_EMAIL", "cupel@example.com")
        .env("GIT_COMMITTER_NAME", "Cupel")
        .env("GIT_COMMITTER_EMAIL", "cupel@example.com")
        .output()
        .unwrap();
    assert!(status.status.success(), "git {args:?}: {status:?}");
}

/// The fixture of `lang` committed and tagged `v1` with `file` as `old` (rounding toward zero),
/// then changed back to the fixture's `quarter`, and compared with the `ref` oracle `v1`.
fn assert_ref_divergence(lang: &str, file: &str, old: &str) {
    let dir = tempfile::tempdir().unwrap();
    let (table, _) = fixture(lang, dir.path());
    let now = fs::read_to_string(dir.path().join(file)).unwrap();
    fs::write(dir.path().join(file), old).unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "v1"]);
    git(dir.path(), &["tag", "v1"]);
    fs::write(dir.path().join(file), now).unwrap();
    let oracle = RunOracle {
        name: "release".to_owned(),
        definition: OracleConfig {
            kind: OracleKind::Ref,
            reference: Some("v1".to_owned()),
            cmd: None,
            spec: None,
            lang: None,
            inputs: Vec::new(),
        },
    };
    let ctx = run_ctx(lang, dir.path(), &table, Some(oracle), Some(10));
    let Some(results) = compared(lang, &ctx) else {
        return;
    };
    let results = results.unwrap();
    let commit = results
        .oracle
        .as_ref()
        .and_then(|oracle| oracle.commit.clone());
    assert_eq!(commit.map(|sha| sha.len()), Some(40), "{results:#?}");
    assert_quarter_diverges_at_minus_one(&results.entries[0], 2, 10);
    assert_eq!(
        fs::read_to_string(dir.path().join(file)).unwrap(),
        now_of(lang)
    );
}

/// The fixture's own `quarter` file for `lang`: the working tree is never written.
fn now_of(lang: &str) -> String {
    let file = match lang {
        "python" => "quarter.py",
        "javascript" => "quarter.js",
        "typescript" => "src/quarter.ts",
        "go" => "quarter.go",
        _ => "src/lib.rs",
    };
    fs::read_to_string(cupel_fixtures::repo_path(&format!("gen.differential/{lang}")).join(file))
        .unwrap()
}

#[test]
fn python_diverges_from_the_commit_it_changed() {
    assert_ref_divergence(
        "python",
        "quarter.py",
        "def quarter(cents: int) -> int:\n    return int(cents / 4)\n",
    );
}

#[test]
fn javascript_diverges_from_the_commit_it_changed() {
    assert_ref_divergence(
        "javascript",
        "quarter.js",
        "export function quarter(cents) {\n  return Math.trunc(cents / 4);\n}\n",
    );
}

#[test]
fn typescript_diverges_from_the_commit_it_changed() {
    assert_ref_divergence(
        "typescript",
        "src/quarter.ts",
        "export function quarter(cents: number): number {\n  return Math.trunc(cents / 4);\n}\n",
    );
}

#[test]
fn go_diverges_from_the_commit_it_changed() {
    assert_ref_divergence(
        "go",
        "quarter.go",
        "package quarter\n\nfunc Quarter(cents int64) int64 {\n\treturn cents / 4\n}\n",
    );
}

#[test]
fn rust_diverges_from_the_commit_it_changed() {
    assert_ref_divergence(
        "rust",
        "src/lib.rs",
        "pub fn quarter(cents: i64) -> i64 {\n    cents / 4\n}\n",
    );
}

/// A Python repository whose `step.py` holds `source`, with the entry `step.py:step` and a
/// command oracle `cmd`.
fn python_repo(dir: &Path, source: &str, cmd: &str) -> ProduceCtx {
    fs::write(dir.join("step.py"), source).unwrap();
    let table = json!({"differential_entries": ["step.py:step"]})
        .as_object()
        .unwrap()
        .clone();
    let oracle = RunOracle {
        name: "identity".to_owned(),
        definition: OracleConfig {
            kind: OracleKind::Command,
            reference: None,
            cmd: Some(cmd.to_owned()),
            spec: None,
            lang: None,
            inputs: Vec::new(),
        },
    };
    run_ctx("python", dir, &table, Some(oracle), None)
}

const IDENTITY: &str = "python3 -c 'import sys; print(int(sys.argv[1]))' {input}";

#[test]
fn a_divergence_found_at_a_boundary_is_minimized_to_the_smallest_input_that_still_diverges() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = python_repo(
        dir.path(),
        "def step(x: int) -> int:\n    return x if x < 50 else x + 1\n",
        IDENTITY,
    );
    let Some(entry) = only_entry("python", &ctx) else {
        return;
    };
    let divergence = entry.divergence.unwrap();
    assert_eq!(
        (divergence.found, divergence.case, divergence.input),
        (vec![json!(i64::MAX)], 4, vec![json!(50)])
    );
    assert!(divergence.shrinks > 1, "{}", divergence.shrinks);
    assert_eq!(
        (divergence.implementation, divergence.oracle),
        (
            Answer::Returned("51".to_owned()),
            Answer::Returned("50".to_owned())
        )
    );
}

#[test]
fn the_same_seed_compares_the_same_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = python_repo(
        dir.path(),
        "def step(x: int) -> int:\n    return x\n",
        IDENTITY,
    );
    let Some(first) = compared("python", &ctx) else {
        return;
    };
    let first = first.unwrap();
    assert_eq!(compared("python", &ctx).unwrap().unwrap(), first);
    assert_eq!((first.seed, first.cases), (SEED, 100));
    assert_eq!(
        (first.entries[0].compared, &first.entries[0].divergence),
        (100, &None)
    );
}

#[test]
fn cases_sets_how_many_inputs_are_generated_beside_the_known_ones() {
    let dir = tempfile::tempdir().unwrap();
    let (table, oracle) = fixture("python", dir.path());
    let ctx = run_ctx("python", dir.path(), &table, Some(oracle), Some(0));
    let Some(entry) = only_entry("python", &ctx) else {
        return;
    };
    assert_eq!((entry.compared, entry.divergence), (2, None));
}

#[test]
fn an_entry_that_cannot_be_called_is_skipped_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let mut ctx = python_repo(
        dir.path(),
        "def step(x: dict) -> int:\n    return 0\n\ndef _hidden(x: int) -> int:\n    return x\n",
        IDENTITY,
    );
    let entries = json!(["step.py:step", "step.py:missing", "step.py:_hidden"]);
    ctx.config
        .insert("differential_entries".to_owned(), entries);
    let Some(results) = compared("python", &ctx) else {
        return;
    };
    let skipped: Vec<Option<String>> = results
        .unwrap()
        .entries
        .into_iter()
        .map(|entry| entry.skipped)
        .collect();
    assert_eq!(
        skipped,
        vec![
            Some("no generator for the parameter type `dict`".to_owned()),
            Some("no function `missing` in `step.py`".to_owned()),
            Some("`_hidden` is not public: the harness calls what a module exports".to_owned()),
        ]
    );
}

#[test]
fn a_function_that_raises_or_exits_is_answered_and_the_rest_still_run() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = python_repo(
        dir.path(),
        "import sys\n\ndef step(x: int) -> int:\n    if x == 1:\n        raise ValueError('one')\n    if x == -1:\n        sys.exit(3)\n    return x\n",
        "python3 -c 'import sys; x = int(sys.argv[1]); sys.exit(1) if x == 1 else print(x)' {input}",
    );
    let Some(entry) = only_entry("python", &ctx) else {
        return;
    };
    let divergence = entry.divergence.unwrap();
    assert_eq!(
        (
            divergence.input,
            divergence.implementation,
            divergence.oracle
        ),
        (
            vec![json!(-1)],
            Answer::Crashed("exit 3".to_owned()),
            Answer::Returned("-1".to_owned())
        )
    );
}

#[test]
fn without_an_oracle_or_with_a_spec_nothing_is_compared() {
    let dir = tempfile::tempdir().unwrap();
    let (table, _) = fixture("python", dir.path());
    let none = compared("python", &run_ctx("python", dir.path(), &table, None, None));
    let spec = RunOracle {
        name: "model".to_owned(),
        definition: OracleConfig {
            kind: OracleKind::Spec,
            reference: None,
            cmd: None,
            spec: Some("model.qnt".to_owned()),
            lang: Some("quint".to_owned()),
            inputs: Vec::new(),
        },
    };
    let specified = compared(
        "python",
        &run_ctx("python", dir.path(), &table, Some(spec), None),
    );
    let (Some(none), Some(specified)) = (none, specified) else {
        return;
    };
    let (none, specified) = (none.unwrap(), specified.unwrap());
    assert_eq!((none.oracle, none.entries.len()), (None, 0));
    assert_eq!(
        (
            specified.oracle.map(|oracle| oracle.kind),
            specified.entries.len()
        ),
        (Some(OracleKind::Spec), 0)
    );
}

#[test]
fn an_oracle_that_cannot_be_reached_stops_the_comparison() {
    let dir = tempfile::tempdir().unwrap();
    let (table, mut oracle) = fixture("python", dir.path());
    oracle.definition.cmd = Some("cupel-no-such-oracle {input}".to_owned());
    let missing = compared(
        "python",
        &run_ctx("python", dir.path(), &table, Some(oracle.clone()), None),
    );
    oracle.definition = OracleConfig {
        kind: OracleKind::Ref,
        reference: Some("v9".to_owned()),
        cmd: None,
        spec: None,
        lang: None,
        inputs: Vec::new(),
    };
    let unknown = compared(
        "python",
        &run_ctx("python", dir.path(), &table, Some(oracle), None),
    );
    let (Some(missing), Some(unknown)) = (missing, unknown) else {
        return;
    };
    assert!(
        matches!(&missing, Err(Error::Process { program, message })
            if program == "sh" && message.starts_with("the oracle's program is not installed")),
        "{missing:?}"
    );
    assert!(
        matches!(&unknown, Err(Error::Process { program, .. }) if program == "git"),
        "{unknown:?}"
    );
}

#[test]
fn a_cancelled_run_stops_the_engine() {
    let dir = tempfile::tempdir().unwrap();
    let (table, oracle) = fixture("python", dir.path());
    let mut ctx = run_ctx("python", dir.path(), &table, Some(oracle), None);
    let cancel = Cancellation::new();
    cancel.cancel();
    ctx.cancel = cancel;
    let Some(outcome) = compared("python", &ctx) else {
        return;
    };
    assert_eq!(outcome.unwrap_err(), Error::Cancelled);
}

#[test]
fn the_engine_is_unconfigured_until_the_language_lists_entry_points() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        engine("go").availability(&Ctx::new(dir.path().to_path_buf())),
        Availability::Unconfigured {
            key: "lang.go.differential_entries".to_owned()
        }
    );
    assert_eq!(
        engine("rust").needs(),
        [FactKind::Symbols, FactKind::ApiSurface]
    );
}
