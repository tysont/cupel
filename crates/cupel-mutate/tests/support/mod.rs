// ABOUTME: Helpers the engine tests share: a copy of a mutation fixture, and `MutationOutcomes` produced there the way
// ABOUTME: a run produces it (the scheduler's fact store, the baseline gate, every registered producer).
#![allow(
    dead_code,
    reason = "each test binary uses a different subset of these helpers"
)]

use std::path::{Path, PathBuf};

use cupel_core::config::{Config, ConfigRequest};
use cupel_core::syntax::ProjectInfo;
use cupel_core::{
    Cancellation, ChangedLines, CostClass, Error, FactKind, Job, Lang, MutantOutcome,
    MutationOutcomes, RunId, Scheduler, Writer,
};
use cupel_fixtures::matrix::Registries;

/// The registries the binary uses, built once.
pub fn registries() -> &'static Registries {
    Registries::shared()
}

/// The registered language `id`.
pub fn lang(id: &str) -> Lang {
    Registries::shared().lang(id)
}

/// A copy of `repos/<name>`.
pub fn fixture(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    cupel_fixtures::copy_repo(name, dir.path());
    dir
}

/// What a run at `depth`, over a range that changed `changed`, with `--config` `config`, would
/// see.
#[derive(Debug, Clone, Default)]
pub struct Run {
    pub depth: Option<CostClass>,
    pub changed: Option<ChangedLines>,
    pub config: Option<PathBuf>,
}

/// `MutationOutcomes` for `lang` in the repository at `root`, produced as a `check` run would.
pub fn produce(root: &Path, lang: &str, run: &Run) -> Result<MutationOutcomes, Error> {
    let config = Config::load(&ConfigRequest {
        root: root.to_path_buf(),
        config_path: run.config.clone(),
        ..ConfigRequest::default()
    })
    .unwrap();
    let writer = Writer::new(root, false).unwrap();
    let registries = registries();
    let files = cupel_lang::walk(root, &registries.languages, &ProjectInfo::default());
    let job = Job {
        run: RunId::generate(),
        writer: &writer,
        cancel: Cancellation::new(),
        config: &config,
        selected: Vec::new(),
        langs: vec![self::lang(lang)],
        files,
        selectors: Vec::new(),
        selectors_must_match: false,
        base: None,
        seed: 0,
        depth: run.depth.unwrap_or(CostClass::Standard),
        changed: run.changed.clone(),
        resources: cupel_core::budget::Resources::default(),
        oracle: None,
        cases: None,
    };
    let scheduler = Scheduler {
        checks: &registries.checks,
        producers: &registries.producers,
    };
    let mut production = scheduler.production(&job);
    let fact = production.ensure(
        FactKind::MutationOutcomes,
        &self::lang(lang),
        &mut Vec::new(),
    )?;
    Ok(fact.downcast_ref::<MutationOutcomes>().unwrap().clone())
}

/// Each mutant as `line from→to outcome`, with ` (equivalent?)` when it is marked.
pub fn summary(outcomes: &MutationOutcomes) -> Vec<String> {
    outcomes
        .mutants
        .iter()
        .map(|mutant| {
            format!(
                "{} {}→{} {}{}",
                mutant.span.line,
                mutant.from,
                mutant.to,
                mutant.outcome,
                if mutant.equivalent.is_some() {
                    " (equivalent?)"
                } else {
                    ""
                }
            )
        })
        .collect()
}

/// The mutants of `outcomes` with `outcome`, as `line from→to`.
pub fn with_outcome(outcomes: &MutationOutcomes, outcome: MutantOutcome) -> Vec<String> {
    outcomes
        .mutants
        .iter()
        .filter(|mutant| mutant.outcome == outcome)
        .map(|mutant| format!("{} {}→{}", mutant.span.line, mutant.from, mutant.to))
        .collect()
}

/// A copy of the Python mutation fixture and its outcomes at the default depth; `None` (reported
/// as skipped) without pytest.
pub fn python() -> Option<(tempfile::TempDir, MutationOutcomes)> {
    python_at(&Run::default())
}

/// [`python`] in a run like `run`.
fn python_at(run: &Run) -> Option<(tempfile::TempDir, MutationOutcomes)> {
    if !installed("pytest", "the Python mutation fixture") {
        return None;
    }
    let dir = fixture("tests.mutation/python");
    let outcomes = produce(dir.path(), "python", run).unwrap();
    Some((dir, outcomes))
}

/// A run at `deep`, the depth that measures a per-test map (its producers cost `deep`).
pub fn deep() -> Run {
    Run {
        depth: Some(CostClass::Deep),
        ..Run::default()
    }
}

/// [`python`] at `deep`, when its mutants ran by a per-test map; `None` (reported as skipped)
/// without pytest-cov.
pub fn python_per_test() -> Option<(tempfile::TempDir, MutationOutcomes)> {
    python_at(&deep()).filter(|(_, outcomes)| {
        if !outcomes.per_test {
            eprintln!("SKIPPED: pytest-cov not installed; per-test selection was not run");
        }
        outcomes.per_test
    })
}

/// Whether `program` is on `PATH`; prints the skip line when it is not.
pub fn installed(program: &str, what: &str) -> bool {
    let found = cupel_core::find_program(program).is_some();
    if !found {
        eprintln!("SKIPPED: {program} not installed; {what} was not run");
    }
    found
}
