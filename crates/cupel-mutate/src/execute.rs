// ABOUTME: Running mutants (ADR-080 §8–9, ADR-082, ADR-147): the mutants of one file run concurrently, one slot each,
// ABOUTME: after a green reference run (of the unmutated file, or of its validated schemata build); each is killed or not.
use std::collections::BTreeSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use cupel_core::{
    Ctx, Error, MutantOutcome, MutationStrategy, ProduceCtx, Result, RunOptions, Runner, TestRun,
    TestSelection, WriteError, Writer,
};

use crate::overlay::Slot;
use crate::plan::Planned;
use crate::settings::Settings;

/// The environment variable that selects a schemata mutant at run time (ADR-082); `0` selects
/// none.
pub const MUTANT_VAR: &str = "CUPEL_MUTANT";

/// What every mutant run of one production shares.
pub struct Execution<'a> {
    pub ctx: &'a ProduceCtx,
    pub runner: &'a dyn Runner,
    pub slots: &'a [Slot],
    /// How long a mutant's run may take (`Settings::timeout`).
    pub settings: Settings,
    /// What a mutant that per-test selection did not narrow runs: the whole suite, or all of it
    /// but the slow tests the runner can leave out (ADR-147).
    pub whole: &'a TestSelection,
}

/// What a mutant's run found.
#[derive(Debug, Clone)]
pub struct Ran {
    pub outcome: MutantOutcome,
    pub tests: Vec<String>,
    pub killing_tests: Vec<String>,
    pub duration_ms: u64,
    pub strategy: MutationStrategy,
    /// Why it was built as a file copy in a file with schemata.
    pub fallback: Option<String>,
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

/// How a file's mutants are built.
pub enum Build<'a> {
    /// Each mutant written into the file and rebuilt.
    Copies,
    /// The validated schemata `source` every slot already holds; `ids[i]` selects mutant `i`
    /// (`None`: a file copy), and slot `n` times out at `limits[n]`.
    Schemata {
        source: &'a str,
        ids: &'a [Option<u32>],
        limits: &'a [Duration],
    },
}

/// Whether a schemata build passed the file's tests with no mutant selected in every slot.
pub enum Validation {
    /// Green everywhere: each slot's timeout.
    Built(Vec<Duration>),
    /// Red somewhere: the first red slot's output, and why it was red.
    Failed { output: String, reason: String },
}

/// What one slot has written as the mutated file.
#[derive(PartialEq, Eq)]
enum Holds {
    Original,
    /// The original with one more trailing newline, as a reference run leaves it.
    Renewed,
    Schemata,
}

impl Execution<'_> {
    /// Writes `source` (a schemata build of `run.file`) into every slot `run` uses and runs the
    /// file's tests against it with no mutant selected: the build every gated mutant shares.
    pub fn validate(&self, run: &FileRun<'_>, source: &str) -> Result<Validation> {
        let slots = &self.slots[..self.workers(run)];
        let ran: Vec<Result<(TestRun, Duration)>> = std::thread::scope(|scope| {
            let handles: Vec<_> = slots
                .iter()
                .map(|slot| {
                    scope.spawn(move || {
                        let ctx = self.slot_ctx(slot)?;
                        for (importer, text) in run.importers {
                            write(slot, importer, text)?;
                        }
                        write(slot, run.file, source.as_bytes())?;
                        self.timed(&ctx, &self.union(&run.mutants))
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap_or_else(|_| Err(panicked())))
                .collect()
        });
        let mut limits = Vec::new();
        for ran in ran {
            let (ran, elapsed) = ran?;
            if !ran.results.is_green() {
                return Ok(Validation::Failed {
                    output: output(&ran),
                    reason: why_red(&ran),
                });
            }
            limits.push(self.limit(elapsed));
        }
        Ok(Validation::Built(limits))
    }

    /// Runs every mutant of `run` across the slots, each slot taking the next mutant when it is
    /// free, gated mutants first; the outcomes in the order of `run.mutants`.
    pub fn run_file(&self, run: &FileRun<'_>, build: &Build<'_>) -> Result<Vec<Ran>> {
        let mut order: Vec<usize> = (0..run.mutants.len()).collect();
        order.sort_by_key(|index| id(build, *index).is_none());
        let next = AtomicUsize::new(0);
        let found: Mutex<Vec<Option<Ran>>> = Mutex::new(vec![None; run.mutants.len()]);
        let work = Work {
            run,
            build,
            order: &order,
            next: &next,
            found: &found,
        };
        let failures: Vec<Error> = std::thread::scope(|scope| {
            let handles: Vec<_> = self.slots[..self.workers(run)]
                .iter()
                .enumerate()
                .map(|(index, slot)| {
                    let work = &work;
                    scope.spawn(move || self.work(index, slot, work))
                })
                .collect();
            handles
                .into_iter()
                .filter_map(|handle| handle.join().unwrap_or_else(|_| Err(panicked())).err())
                .collect()
        });
        if let Some(failure) = failures.into_iter().next() {
            return Err(failure);
        }
        let found = found.into_inner().unwrap_or_default();
        Ok(found.into_iter().flatten().collect())
    }

    /// Writes `run`'s original file back into every slot it uses, after its mutants ended in an
    /// error (the budget, a cancellation, a reference that is not green), so no slot keeps a
    /// mutant or a schemata build of it (ADR-141 §3). A slot that cannot be written is made
    /// whole by its next sync.
    pub fn restore(&self, run: &FileRun<'_>) {
        for slot in &self.slots[..self.workers(run)] {
            let _ = slot.write(run.file, run.original);
        }
    }

    /// The slots `run` uses: one per mutant, at most all of them.
    fn workers(&self, run: &FileRun<'_>) -> usize {
        self.slots.len().min(run.mutants.len())
    }

    /// One slot's work on a file: the reference (unless the schemata build was validated),
    /// then mutants until none is left, then the original file back.
    fn work(&self, index: usize, slot: &Slot, work: &Work<'_>) -> Result<()> {
        let (run, build) = (work.run, work.build);
        let ctx = self.slot_ctx(slot)?;
        let (limit, holds) = match build {
            Build::Copies => {
                for (importer, text) in run.importers {
                    write(slot, importer, text)?;
                }
                (self.reference(slot, &ctx, run)?, Holds::Renewed)
            }
            Build::Schemata { limits, .. } => (limits[index], Holds::Schemata),
        };
        let mut worker = Worker {
            slot,
            ctx,
            limit,
            holds,
        };
        while let Some(&mutant) = work.order.get(work.next.fetch_add(1, Ordering::SeqCst)) {
            let ran = self.one(&mut worker, work, mutant)?;
            if let Ok(mut found) = work.found.lock() {
                found[mutant] = Some(ran);
            }
        }
        if worker.holds != Holds::Original {
            write(slot, run.file, run.original)?;
        }
        Ok(())
    }

    /// Runs mutant `index` of the file in `worker`'s slot: selected by its id in the schemata
    /// build, or written as a file copy and the original put back.
    fn one(&self, worker: &mut Worker<'_>, work: &Work<'_>, index: usize) -> Result<Ran> {
        let (run, planned, slot) = (work.run, work.run.mutants[index], worker.slot);
        if let (Some(id), Build::Schemata { source, .. }) = (id(work.build, index), work.build) {
            if worker.holds != Holds::Schemata {
                write(slot, run.file, source.as_bytes())?;
                worker.holds = Holds::Schemata;
            }
            return self.mutant(&worker.ctx, planned, worker.limit, id);
        }
        write(slot, run.file, planned.source.as_bytes())?;
        let ran = self.mutant(&worker.ctx, planned, worker.limit, 0);
        write(slot, run.file, run.original)?;
        worker.holds = Holds::Original;
        ran
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

    /// Runs the tests of `run`'s mutants against the unmutated file with one more trailing
    /// newline, which builds and loads like the original but is new to every build cache (by
    /// content or by time), so the run rebuilds what a mutant would; the timeout its mutants
    /// get. A reference that is not green means the slot does not reproduce the baseline.
    fn reference(&self, slot: &Slot, ctx: &ProduceCtx, run: &FileRun<'_>) -> Result<Duration> {
        let mut renewed = run.original.to_vec();
        renewed.push(b'\n');
        write(slot, run.file, &renewed)?;
        let (ran, elapsed) = self.timed(ctx, &self.union(&run.mutants))?;
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
        Ok(self.limit(elapsed))
    }

    /// Runs `selection` with no mutant selected; the run and its wall time.
    fn timed(&self, ctx: &ProduceCtx, selection: &TestSelection) -> Result<(TestRun, Duration)> {
        let started = Instant::now();
        let ran = self.runner.run(ctx, selection, &self.options(None, 0))?;
        Ok((ran, started.elapsed()))
    }

    /// A mutant's timeout after a reference run of `elapsed`.
    fn limit(&self, elapsed: Duration) -> Duration {
        self.settings.timeout(elapsed)
    }

    /// The tests every mutant of a file runs together: the whole selection when one of them
    /// runs it.
    fn union(&self, mutants: &[&Planned]) -> TestSelection {
        let mut ids = BTreeSet::new();
        for planned in mutants {
            match &planned.tests {
                Some(tests) => ids.extend(tests.iter().cloned()),
                None => return self.whole.clone(),
            }
        }
        TestSelection::Tests(ids.into_iter().collect())
    }

    /// How every run of the production runs: in the runner's own order, with the run's seed (so
    /// `--seed` repeats it, and nothing is answered from a cache of earlier results), `timeout`,
    /// and `CUPEL_MUTANT` set to `mutant` (0 for none), so a variable inherited from an outer run
    /// never selects a mutant here.
    fn options(&self, timeout: Option<Duration>, mutant: u32) -> RunOptions {
        RunOptions {
            env: vec![(MUTANT_VAR.to_owned(), mutant.to_string())],
            timeout,
            seed: Some(self.ctx.seed),
            ..RunOptions::default()
        }
    }

    /// Runs `planned`'s tests against it (already written, or selected by `id`) with `limit`.
    fn mutant(&self, ctx: &ProduceCtx, planned: &Planned, limit: Duration, id: u32) -> Result<Ran> {
        let selection = planned
            .tests
            .clone()
            .map_or_else(|| self.whole.clone(), TestSelection::Tests);
        let started = Instant::now();
        let ran = self
            .runner
            .run(ctx, &selection, &self.options(Some(limit), id))?;
        let elapsed = started.elapsed();
        let strategy = if id == 0 {
            MutationStrategy::FileCopy
        } else {
            MutationStrategy::Schemata
        };
        Ok(classify(&ran, elapsed >= limit, elapsed, strategy))
    }
}

/// One slot at work on a file: where it runs, its timeout, and what the file holds now.
struct Worker<'a> {
    slot: &'a Slot,
    ctx: ProduceCtx,
    limit: Duration,
    holds: Holds,
}

/// What the slots working on one file share.
struct Work<'a> {
    run: &'a FileRun<'a>,
    build: &'a Build<'a>,
    /// Indexes of `run.mutants` in the order they are taken.
    order: &'a [usize],
    next: &'a AtomicUsize,
    found: &'a Mutex<Vec<Option<Ran>>>,
}

/// The schemata id of mutant `index` under `build`; `None` for a file copy.
fn id(build: &Build<'_>, index: usize) -> Option<u32> {
    match build {
        Build::Copies => None,
        Build::Schemata { ids, .. } => ids.get(index).copied().flatten(),
    }
}

/// What a finished mutant run says: killed when a test failed, a timeout when it ran out of
/// time, survived when every invocation completed, else unviable (it did not build or load).
fn classify(
    ran: &TestRun,
    out_of_time: bool,
    elapsed: Duration,
    strategy: MutationStrategy,
) -> Ran {
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
        strategy,
        fallback: None,
    }
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

/// Everything `ran`'s incomplete invocations printed, where a build names its errors.
fn output(ran: &TestRun) -> String {
    ran.results
        .incomplete()
        .filter_map(|invocation| invocation.output.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

fn panicked() -> Error {
    Error::Process {
        program: "engine.mutation".to_owned(),
        message: "a mutation worker panicked".to_owned(),
    }
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
