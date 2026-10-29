// ABOUTME: Running mutants (ADR-080 §8–9, ADR-082, ADR-147, ADR-148, ADR-150): the mutants of one file run concurrently,
// ABOUTME: one slot each with its share of the cores and memory, after a green reference run, each phase timed.
use std::collections::BTreeSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use cupel_core::{
    Ctx, Error, MutantOutcome, MutationPhase, MutationStrategy, ProduceCtx, Result, RunOptions,
    Runner, TestRun, TestSelection, WriteError, Writer, physical_memory,
};

use crate::overlay::Slot;
use crate::phases::{Phases, Served};
use crate::plan::Planned;
use crate::settings::{Settings, threads};

/// The environment variable that selects a schemata mutant at run time (ADR-082); `0` selects
/// none.
pub const MUTANT_VAR: &str = "CUPEL_MUTANT";

/// What every mutant run of one production shares.
pub struct Execution<'a> {
    pub ctx: &'a ProduceCtx,
    pub runner: &'a dyn Runner,
    pub slots: &'a [Slot],
    /// How long a mutant's run may take (`Settings::timeout`) and how much memory the slots
    /// may hold together (`Settings::memory_per_slot`).
    pub settings: Settings,
    /// What a mutant that per-test selection did not narrow runs: the whole suite, or all of it
    /// but the slow tests the runner can leave out (ADR-147).
    pub whole: &'a TestSelection,
    /// Where every phase is recorded (ADR-150).
    pub phases: &'a Phases,
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
    ///
    /// When a slot it uses has no build of its own, the first slot validates alone first, with
    /// every core and the whole memory budget, and each slot with no build gets a copy of the
    /// first slot's before its own validation (ADR-150 §6).
    pub fn validate(&self, run: &FileRun<'_>, source: &str) -> Result<Validation> {
        let slots = &self.slots[..self.workers(run)];
        let outputs = self.runner.outputs();
        let mut ran = Vec::new();
        let mut rest = 0;
        if slots.len() > 1 && slots.iter().any(|slot| !slot.built(outputs)) {
            let first = self.validate_in(0, &slots[0], run, source, Share::Alone)?;
            let green = first.0.results.is_green();
            ran.push(Ok(first));
            if !green {
                return self.validation(ran);
            }
            for (index, slot) in slots.iter().enumerate().skip(1) {
                if !slot.built(outputs) {
                    self.seed(index, slot, &slots[0], run)?;
                }
            }
            rest = 1;
        }
        ran.extend(std::thread::scope(|scope| {
            let handles: Vec<_> = slots
                .iter()
                .enumerate()
                .skip(rest)
                .map(|(index, slot)| {
                    scope.spawn(move || self.validate_in(index, slot, run, source, Share::Slot))
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap_or_else(|_| Err(panicked())))
                .collect::<Vec<_>>()
        }));
        self.validation(ran)
    }

    /// What the validation runs `ran` say: each slot's timeout when every one is green, else
    /// the first red one's output and why.
    fn validation(&self, ran: Vec<Result<(TestRun, Duration)>>) -> Result<Validation> {
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

    /// Writes `source` and `run`'s importers into slot `index` and validates it there with
    /// `share` of the machine.
    fn validate_in(
        &self,
        index: usize,
        slot: &Slot,
        run: &FileRun<'_>,
        source: &str,
        share: Share,
    ) -> Result<(TestRun, Duration)> {
        let ctx = self.slot_ctx(slot)?;
        for (importer, text) in run.importers {
            write(slot, importer, text)?;
        }
        write(slot, run.file, source.as_bytes())?;
        self.build(index, &ctx, run, share)
    }

    /// Copies `from`'s build into slot `index`, which has none; the `seed` phase.
    fn seed(&self, index: usize, slot: &Slot, from: &Slot, run: &FileRun<'_>) -> Result<()> {
        let started = Instant::now();
        slot.seed(from, self.runner.outputs()).map_err(|error| {
            Error::Write(WriteError::Io {
                path: slot.root().to_path_buf(),
                message: error.to_string(),
            })
        })?;
        let served = Served {
            file: Some(run.file),
            slot: Some(index),
            tests: None,
        };
        self.phases.record(MutationPhase::Seed, served, started);
        Ok(())
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
                (self.reference(index, slot, &ctx, run)?, Holds::Renewed)
            }
            Build::Schemata { limits, .. } => (limits[index], Holds::Schemata),
        };
        let mut worker = Worker {
            index,
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
            return self.mutant(worker, planned, id);
        }
        write(slot, run.file, planned.source.as_bytes())?;
        let ran = self.mutant(worker, planned, 0);
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
    fn reference(
        &self,
        index: usize,
        slot: &Slot,
        ctx: &ProduceCtx,
        run: &FileRun<'_>,
    ) -> Result<Duration> {
        let mut renewed = run.original.to_vec();
        renewed.push(b'\n');
        write(slot, run.file, &renewed)?;
        let (ran, elapsed) = self.build(index, ctx, run, Share::Slot)?;
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

    /// The build every mutant of `run`'s file shares, in slot `index` (`ctx` is the slot's): the
    /// file's selected tests with no mutant, timed and recorded as the `build` phase. A run over
    /// the slot's memory budget is an error naming the setting: every mutant of the file would
    /// be.
    fn build(
        &self,
        index: usize,
        ctx: &ProduceCtx,
        run: &FileRun<'_>,
        share: Share,
    ) -> Result<(TestRun, Duration)> {
        let selection = self.union(&run.mutants);
        let started = Instant::now();
        let ran = match self
            .runner
            .run(ctx, &selection, &self.options_for(None, 0, share))
        {
            Err(Error::OutOfMemory { limit }) => {
                return Err(Error::Process {
                    program: self.runner.id().to_string(),
                    message: format!(
                        "the tests of the unmutated file held more than {} MiB of memory, one \
                         slot's share of `max_mutant_memory`; raise it or lower `mutation.jobs`",
                        limit >> 20
                    ),
                });
            }
            ran => ran?,
        };
        let served = Served {
            file: Some(run.file),
            slot: Some(index),
            tests: selected(&selection),
        };
        self.phases.record(MutationPhase::Build, served, started);
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
    /// this slot's share of the cores and of the memory budget (ADR-148), and `CUPEL_MUTANT`
    /// set to `mutant` (0 for none), so a variable inherited from an outer run never selects a
    /// mutant here.
    fn options(&self, timeout: Option<Duration>, mutant: u32) -> RunOptions {
        self.options_for(timeout, mutant, Share::Slot)
    }

    /// [`Self::options`] for a run with `share` of the machine: a slot's share, or all of it for
    /// a run nothing else of the production runs beside (ADR-150 §6).
    fn options_for(&self, timeout: Option<Duration>, mutant: u32, share: Share) -> RunOptions {
        let slots = match share {
            Share::Slot => self.slots.len(),
            Share::Alone => 1,
        };
        let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        RunOptions {
            env: vec![(MUTANT_VAR.to_owned(), mutant.to_string())],
            timeout,
            memory: self.settings.memory_per_slot(physical_memory(), slots),
            seed: Some(self.ctx.seed),
            parallelism: Some(threads(cores, slots)),
            ..RunOptions::default()
        }
    }

    /// Runs `planned`'s tests against it (already written in `worker`'s slot, or selected by
    /// `id`) with the worker's timeout; the `run` phase.
    fn mutant(&self, worker: &Worker<'_>, planned: &Planned, id: u32) -> Result<Ran> {
        let selection = planned
            .tests
            .clone()
            .map_or_else(|| self.whole.clone(), TestSelection::Tests);
        let limit = worker.limit;
        let started = Instant::now();
        let strategy = if id == 0 {
            MutationStrategy::FileCopy
        } else {
            MutationStrategy::Schemata
        };
        let ran = self
            .runner
            .run(&worker.ctx, &selection, &self.options(Some(limit), id));
        let elapsed = started.elapsed();
        let served = Served {
            file: Some(planned.file()),
            slot: Some(worker.index),
            tests: selected(&selection),
        };
        self.phases.record(MutationPhase::Run, served, started);
        match ran {
            Err(Error::OutOfMemory { .. }) => Ok(out_of_memory(elapsed, strategy)),
            Err(error) => Err(error),
            Ok(ran) => Ok(classify(&ran, elapsed >= limit, elapsed, strategy)),
        }
    }
}

/// How much of the machine a run gets: its slot's share of the cores and the memory budget,
/// or all of both when it runs alone (ADR-148 §1, ADR-150 §6).
#[derive(Clone, Copy)]
enum Share {
    Slot,
    Alone,
}

/// How many tests `selection` names; `None` for the whole suite, however narrowed.
fn selected(selection: &TestSelection) -> Option<usize> {
    match selection {
        TestSelection::Tests(ids) | TestSelection::Files(ids) => Some(ids.len()),
        TestSelection::All | TestSelection::Except(_) => None,
    }
}

/// One slot at work on a file: where it runs, its timeout, and what the file holds now.
struct Worker<'a> {
    /// The slot's index among the production's.
    index: usize,
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

/// What a mutant run killed over its memory budget says: nothing ran to report (ADR-148).
fn out_of_memory(elapsed: Duration, strategy: MutationStrategy) -> Ran {
    Ran {
        outcome: MutantOutcome::OutOfMemory,
        tests: Vec::new(),
        killing_tests: Vec::new(),
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
