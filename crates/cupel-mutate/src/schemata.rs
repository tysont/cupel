// ABOUTME: Mutant schemata (ADR-082): a language's rewriter gates every mutant of a file behind `CUPEL_MUTANT`, so the
// ABOUTME: file builds once; the engine asks `rewriter(lang)` and runs file copies where there is none.
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use cupel_core::syntax::Span;
use cupel_core::{MutationPhase, Result};

use crate::execute::{Build, Execution, FileRun, Ran, Validation};
use crate::phases::Served;

mod rust;

/// How many more times a schemata build that does not compile is rebuilt without the gates on
/// the lines its errors name, before the whole file runs as file copies.
const REBUILDS: usize = 2;

/// One mutant to gate: its id, and its site's span, text, and replacement.
#[derive(Debug, Clone, Copy)]
pub struct Gate<'a> {
    /// 1-based; `CUPEL_MUTANT=<id>` selects it.
    pub id: u32,
    pub span: Span,
    pub from: &'a str,
    pub to: &'a str,
}

/// A file rewritten as schemata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rewritten {
    /// The rewritten file; `None` when no gate could be placed.
    pub source: Option<String>,
    /// The gates left out, by id, with why: they run as file copies.
    pub excluded: BTreeMap<u32, String>,
    /// The line of each placed gate's expression, by id: where the compiler reports it.
    pub lines: BTreeMap<u32, u32>,
}

/// Rewrites one language's files as schemata.
pub trait Rewriter: Sync {
    /// `source` with every gate it can place behind the mutant switch, and the reader appended.
    fn rewrite(&self, source: &str, gates: &[Gate<'_>]) -> Rewritten;

    /// The lines of `file` that the build `output` names in its errors.
    fn culprits(&self, output: &str, file: &str) -> BTreeSet<u32>;
}

/// The schemata rewriter of language `lang`; `None` where its mutants run as file copies.
pub fn rewriter(lang: &str) -> Option<&'static dyn Rewriter> {
    match lang {
        "rust" => Some(&rust::Rust),
        _ => None,
    }
}

/// Runs the mutants of `run`: behind one validated schemata build where `rewriter` places them,
/// as file copies otherwise, each with why it fell back (ADR-082 §5–6).
pub fn run_file(
    execution: &Execution<'_>,
    rewriter: Option<&dyn Rewriter>,
    run: &FileRun<'_>,
) -> Result<Vec<Ran>> {
    let Some(rewriter) = rewriter else {
        return execution.run_file(run, &Build::Copies);
    };
    let gates: Vec<Gate<'_>> = (1..)
        .zip(&run.mutants)
        .map(|(id, planned)| Gate {
            id,
            span: planned.mutant.span,
            from: &planned.mutant.from,
            to: &planned.mutant.to,
        })
        .collect();
    let mut attempt = Attempt {
        execution,
        rewriter,
        run,
        gates: &gates,
        fallback: BTreeMap::new(),
    };
    let failed = match attempt.schemata()? {
        Ok(ran) => return Ok(ran),
        Err(failed) => failed,
    };
    let ran = execution.run_file(run, &Build::Copies)?;
    Ok(with_fallback(
        ran,
        &gates,
        &attempt.fallback,
        failed.as_deref(),
    ))
}

/// Building one file's schemata until it validates or gives up.
struct Attempt<'a> {
    execution: &'a Execution<'a>,
    rewriter: &'a dyn Rewriter,
    run: &'a FileRun<'a>,
    gates: &'a [Gate<'a>],
    /// The gates left to file copies so far, with why.
    fallback: BTreeMap<u32, String>,
}

impl Attempt<'_> {
    /// The mutants' runs behind a validated schemata build; else why the whole file falls back
    /// (`None` when every gate has its own reason).
    fn schemata(&mut self) -> Result<std::result::Result<Vec<Ran>, Option<String>>> {
        let (run, file) = (self.run, self.run.file);
        let text = String::from_utf8_lossy(run.original);
        let mut failed = None;
        for _ in 0..=REBUILDS {
            let open: Vec<Gate<'_>> = self
                .gates
                .iter()
                .filter(|gate| !self.fallback.contains_key(&gate.id))
                .copied()
                .collect();
            let started = Instant::now();
            let rewritten = self.rewriter.rewrite(&text, &open);
            let served = Served {
                file: Some(file),
                ..Served::default()
            };
            self.execution
                .phases
                .record(MutationPhase::Rewrite, served, started);
            self.fallback.extend(rewritten.excluded);
            let Some(source) = rewritten.source else {
                break;
            };
            let (output, reason) = match self.execution.validate(run, &source)? {
                Validation::Built { limits, prebuilt } => {
                    return self.gated(&source, &limits, prebuilt.as_ref()).map(Ok);
                }
                Validation::Failed { output, reason } => (output, reason),
            };
            let lines = self.rewriter.culprits(&output, file);
            let culprits: Vec<(u32, u32)> = rewritten
                .lines
                .into_iter()
                .filter(|(_, line)| lines.contains(line))
                .collect();
            if culprits.is_empty() {
                let first = first_line(&reason);
                return Ok(Err(Some(format!(
                    "the schemata build of `{file}` failed: {first}"
                ))));
            }
            for (id, line) in culprits {
                let why = format!("did not compile under schemata (line {line})");
                self.fallback.insert(id, why);
            }
            failed = Some(format!(
                "the schemata build of `{file}` still failed after {REBUILDS} rebuilds"
            ));
        }
        Ok(Err(failed))
    }

    /// Runs the mutants with `source`, validated with these `limits`, from `prebuilt` when the
    /// runner built once.
    fn gated(
        &self,
        source: &str,
        limits: &[std::time::Duration],
        prebuilt: Option<&cupel_core::Prebuilt>,
    ) -> Result<Vec<Ran>> {
        let ids: Vec<Option<u32>> = self
            .gates
            .iter()
            .map(|gate| (!self.fallback.contains_key(&gate.id)).then_some(gate.id))
            .collect();
        let build = Build::Schemata {
            source,
            ids: &ids,
            limits,
            prebuilt,
        };
        let ran = self.execution.run_file(self.run, &build)?;
        Ok(with_fallback(ran, self.gates, &self.fallback, None))
    }
}

/// `ran` with each file copy's reason: its own, else the whole file's `failed`.
fn with_fallback(
    mut ran: Vec<Ran>,
    gates: &[Gate<'_>],
    fallback: &BTreeMap<u32, String>,
    failed: Option<&str>,
) -> Vec<Ran> {
    for (ran, gate) in ran.iter_mut().zip(gates) {
        if ran.strategy == cupel_core::MutationStrategy::FileCopy {
            ran.fallback = fallback
                .get(&gate.id)
                .cloned()
                .or_else(|| failed.map(str::to_owned));
        }
    }
    ran
}

/// The first non-empty line of `text`, where a build names its first error.
fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|line| line.starts_with("error"))
        .or_else(|| text.lines().map(str::trim).find(|line| !line.is_empty()))
        .unwrap_or_default()
}
