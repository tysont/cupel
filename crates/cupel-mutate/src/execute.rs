// ABOUTME: Running mutants (ADR-080 §8–9): the mutants of one file run concurrently, one slot each, every slot first
// ABOUTME: timing a green reference run of the unmutated file; each mutant's run is then killed, survived, or timed out.
use std::collections::BTreeSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use cupel_core::{
    Ctx, Error, MutantOutcome, ProduceCtx, Result, RunOptions, Runner, TestRun, TestSelection,
    WriteError, Writer,
};

use crate::overlay::Slot;
use crate::plan::Planned;

/// The shortest timeout a mutant's run gets (design §9.3).
const TIMEOUT_FLOOR: Duration = Duration::from_secs(2);

/// What every mutant run of one production shares.
pub struct Execution<'a> {
    pub ctx: &'a ProduceCtx,
    pub runner: &'a dyn Runner,
    pub slots: &'a [Slot],
    pub timeout_factor: u32,
}

/// What a mutant's run found.
#[derive(Debug, Clone)]
pub struct Ran {
    pub outcome: MutantOutcome,
    pub tests: Vec<String>,
    pub killing_tests: Vec<String>,
    pub duration_ms: u64,
}

/// One file's mutants and the files a slot rewrites around them.
pub struct FileRun<'a> {
    /// Relative to the repository root.
    pub file: &'a str,
    /// The file's original text.
    pub original: &'a [u8],
    /// Files that import it relatively, with their text: rewritten fresh in each slot.
    pub importers: &'a [(String, Vec<u8>)],
    pub mutants: Vec<&'a Planned>,
}

impl Execution<'_> {
    /// Runs every mutant of `run` across the slots, each slot taking the next mutant when it is
    /// free; the outcomes in the order of `run.mutants`.
    pub fn run_file(&self, run: &FileRun<'_>) -> Result<Vec<Ran>> {
        let next = AtomicUsize::new(0);
        let found: Mutex<Vec<Option<Ran>>> = Mutex::new(vec![None; run.mutants.len()]);
        let workers = self.slots.len().min(run.mutants.len());
        let failures: Vec<Error> = std::thread::scope(|scope| {
            let handles: Vec<_> = self.slots[..workers]
                .iter()
                .map(|slot| scope.spawn(|| self.work(slot, run, &next, &found)))
                .collect();
            handles
                .into_iter()
                .filter_map(|handle| handle.join().ok().and_then(Result::err))
                .collect()
        });
        if let Some(failure) = failures.into_iter().next() {
            return Err(failure);
        }
        let found = found.into_inner().unwrap_or_default();
        Ok(found.into_iter().flatten().collect())
    }

    /// One slot's work on `run`: the reference, then mutants until none is left.
    fn work(
        &self,
        slot: &Slot,
        run: &FileRun<'_>,
        next: &AtomicUsize,
        found: &Mutex<Vec<Option<Ran>>>,
    ) -> Result<()> {
        let ctx = self.slot_ctx(slot)?;
        for (importer, text) in run.importers {
            write(slot, importer, text)?;
        }
        let limit = self.reference(slot, &ctx, run)?;
        loop {
            let index = next.fetch_add(1, Ordering::SeqCst);
            let Some(planned) = run.mutants.get(index) else {
                return Ok(());
            };
            write(slot, run.file, planned.source.as_bytes())?;
            let ran = self.mutant(&ctx, planned, limit);
            write(slot, run.file, run.original)?;
            let ran = ran?;
            if let Ok(mut found) = found.lock() {
                found[index] = Some(ran);
            }
        }
    }

    /// The context of running in `slot`: its root, and a writer confined to its `.cupel/`.
    fn slot_ctx(&self, slot: &Slot) -> Result<ProduceCtx> {
        let mut ctx = self.ctx.clone();
        ctx.ctx = Ctx {
            root: slot.root().to_path_buf(),
            ..self.ctx.ctx.clone()
        };
        ctx.writer = Writer::new(slot.root(), false)?.confined();
        Ok(ctx)
    }

    /// Runs the tests of `run`'s mutants against the unmutated file, rewritten fresh so the run
    /// rebuilds what a mutant would; the timeout its mutants get. A reference that is not green
    /// means the slot does not reproduce the baseline.
    fn reference(&self, slot: &Slot, ctx: &ProduceCtx, run: &FileRun<'_>) -> Result<Duration> {
        write(slot, run.file, run.original)?;
        let started = Instant::now();
        let ran = self
            .runner
            .run(ctx, &union(&run.mutants), &RunOptions::default())?;
        let elapsed = started.elapsed();
        if !ran.results.is_green() {
            return Err(Error::Process {
                program: self.runner.id().to_string(),
                message: format!(
                    "the unmutated `{}` does not pass its tests in the mutation overlay at {}: {}",
                    run.file,
                    slot.root().display(),
                    why_red(&ran)
                ),
            });
        }
        Ok(TIMEOUT_FLOOR.max(elapsed * self.timeout_factor))
    }

    /// Runs `planned`'s tests against it (already written) with `limit`.
    fn mutant(&self, ctx: &ProduceCtx, planned: &Planned, limit: Duration) -> Result<Ran> {
        let selection = planned
            .tests
            .clone()
            .map_or(TestSelection::All, TestSelection::Tests);
        let options = RunOptions {
            timeout: Some(limit),
            ..RunOptions::default()
        };
        let started = Instant::now();
        let ran = self.runner.run(ctx, &selection, &options)?;
        let elapsed = started.elapsed();
        Ok(classify(&ran, elapsed >= limit, elapsed))
    }
}

/// What a finished mutant run says: killed when a test failed, a timeout when it ran out of
/// time, survived when every invocation completed, else unviable (it did not build or load).
fn classify(ran: &TestRun, out_of_time: bool, elapsed: Duration) -> Ran {
    let results = &ran.results;
    let killing_tests: Vec<String> = results.failures().map(|test| test.id.clone()).collect();
    let complete = results.incomplete().next().is_none();
    let outcome = if !killing_tests.is_empty() {
        MutantOutcome::Killed
    } else if out_of_time && !complete {
        MutantOutcome::Timeout
    } else if complete {
        MutantOutcome::Survived
    } else {
        MutantOutcome::Unviable
    };
    Ran {
        outcome,
        tests: results.tests.iter().map(|test| test.id.clone()).collect(),
        killing_tests,
        duration_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
    }
}

/// The tests every mutant of a file runs together: the whole suite when one of them runs it.
fn union(mutants: &[&Planned]) -> TestSelection {
    let mut ids = BTreeSet::new();
    for planned in mutants {
        match &planned.tests {
            Some(tests) => ids.extend(tests.iter().cloned()),
            None => return TestSelection::All,
        }
    }
    TestSelection::Tests(ids.into_iter().collect())
}

/// The first reason `ran` is red: a failing test or an incomplete invocation's output.
fn why_red(ran: &TestRun) -> String {
    let results = &ran.results;
    if let Some(test) = results.failures().next() {
        return format!("`{}` {}", test.id, test.outcome);
    }
    results
        .incomplete()
        .next()
        .and_then(|invocation| invocation.output.clone())
        .unwrap_or_else(|| "the run reported no tests".to_owned())
}

/// Writes `bytes` as `slot`'s `path`.
fn write(slot: &Slot, path: &str, bytes: &[u8]) -> Result<()> {
    slot.write(path, bytes).map_err(|error| {
        Error::Write(WriteError::Io {
            path: slot.root().join(path),
            message: error.to_string(),
        })
    })
}
