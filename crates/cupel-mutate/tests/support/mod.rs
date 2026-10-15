// ABOUTME: Helpers the engine tests share: a copy of a mutation fixture, and `MutationOutcomes` produced there the way
// ABOUTME: a run produces it (the scheduler's fact store, the baseline gate, every registered producer).
#![allow(
    dead_code,
    reason = "each test binary uses a different subset of these helpers"
)]

use std::path::Path;
use std::sync::OnceLock;

use cupel_core::config::{Config, ConfigRequest};
use cupel_core::syntax::ProjectInfo;
use cupel_core::{
    Cancellation, ChangedLines, CostClass, Error, FactKind, Job, Lang, MutantOutcome,
    MutationOutcomes, RunId, Scheduler, Writer,
};
use cupel_fixtures::matrix::Registries;

/// The registries the binary uses, built once.
pub fn registries() -> &'static Registries {
    static REGISTRIES: OnceLock<Registries> = OnceLock::new();
    REGISTRIES.get_or_init(Registries::builtin)
}

/// The registered language `id`.
pub fn lang(id: &str) -> Lang {
    registries()
        .languages
        .langs()
        .into_iter()
        .find(|lang| lang.id.as_str() == id)
        .unwrap_or_else(|| panic!("{id} is registered"))
}

/// A copy of `repos/<name>`.
pub fn fixture(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    cupel_fixtures::copy_repo(name, dir.path());
    dir
}

/// What a run at `depth`, over a range that changed `changed`, would see.
#[derive(Debug, Clone, Default)]
pub struct Run {
    pub depth: Option<CostClass>,
    pub changed: Option<ChangedLines>,
}

/// `MutationOutcomes` for `lang` in the repository at `root`, produced as a `check` run would.
pub fn produce(root: &Path, lang: &str, run: &Run) -> Result<MutationOutcomes, Error> {
    let config = Config::load(&ConfigRequest {
        root: root.to_path_buf(),
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

/// Whether `program` is on `PATH`; prints the skip line when it is not.
pub fn installed(program: &str, what: &str) -> bool {
    let found = cupel_core::find_program(program).is_some();
    if !found {
        eprintln!("SKIPPED: {program} not installed; {what} was not run");
    }
    found
}
