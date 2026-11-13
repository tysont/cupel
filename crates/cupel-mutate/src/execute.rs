// ABOUTME: Running mutants (ADR-080 §8–9, ADR-082, ADR-147, ADR-148, ADR-150, ADR-160): the mutants of one file run
// ABOUTME: concurrently, one slot each with its share of the cores and memory, a narrowed one's tests one at a time, timed.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

use cupel_core::{
    BuildOutcome, Ctx, Error, MutantOutcome, MutationPhase, MutationStrategy, Prebuilt, ProduceCtx,
    Result, RunOptions, Runner, TestRun, TestSelection, UnfinishedTest, WriteError, Writer,
    physical_memory,
};

use tempfile::TempDir;

use crate::baseline::{self, Baseline};
use crate::overlay::Slot;
use crate::phases::{Phases, Served};
use crate::plan::Planned;
use crate::sequence::{self, Caps, Test, Verdict};
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
    /// Each test's duration in the baseline, which orders a narrowed mutant's tests (ADR-160).
    pub baseline: &'a BTreeMap<String, Duration>,
    /// The unmutated baseline that times every file copy (ADR-185).
    pub reference: &'a Baseline<'a>,
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
    /// The test whose hang killed it (ADR-160).
    pub hung: Option<String>,
    /// For a timeout, the slowest of the tests it had not completed (ADR-160).
    pub slowest: Vec<UnfinishedTest>,
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
        /// The build every slot's gated mutants run from, when the runner builds once.
        prebuilt: Option<&'a Prebuilt>,
    },
}

/// Whether a schemata build passed the file's tests with no mutant selected.
pub enum Validation {
    /// Green: each slot's timeout, and the build its gated mutants run from when the runner
    /// built once (ADR-150 §8).
    Built {
        limits: Vec<Duration>,
        prebuilt: Option<Prebuilt>,
    },
    /// Red: the output of the build or the first red run, and why it was red.
    Failed { output: String, reason: String },
}

/// What one slot has written as the mutated file.
#[derive(PartialEq, Eq)]
enum Holds {
    Original,
    Schemata,
}

impl Execution<'_> {
    /// Writes `source` (a schemata build of `run.file`) where the slots `run` uses will run it
    /// and runs the file's tests against it with no mutant selected: the build every gated
    /// mutant shares.
    ///
    /// With a runner that builds once, the first slot builds alone, with every core and the
    /// whole memory budget, the tests run once from that build, and every slot's mutants run
    /// from it (ADR-150 §8); otherwise every slot builds and validates in parallel.
    pub fn validate(&self, run: &FileRun<'_>, source: &str) -> Result<Validation> {
        let slots = &self.slots[..self.workers(run)];
        let first = &slots[0];
        let ctx = self.slot_ctx(first)?;
        write_file(first, run, source)?;
        let started = Instant::now();
        let tmp = scratch()?;
        let built = match self.runner.build(
            &ctx,
            &within(&self.options_for(None, 0, Share::Alone), &tmp),
        ) {
            Err(Error::OutOfMemory { limit, .. }) => return Err(over_budget(self.runner, limit)),
            built => built?,
        };
        let Some(built) = built else {
            return self.validate_each(run, source, slots);
        };
        let served = Served {
            file: Some(run.file),
            slot: Some(0),
            tests: None,
        };
        self.phases.record(MutationPhase::Build, served, started);
        let prebuilt = match built {
            BuildOutcome::Built(prebuilt) => prebuilt,
            BuildOutcome::Failed { output } => {
                return Ok(Validation::Failed {
                    reason: output.clone(),
                    output,
                });
            }
        };
        let (ran, elapsed) = self.unmutated(
            Unmutated {
                slot: 0,
                file: Some(run.file),
                phase: MutationPhase::Validate,
            },
            &ctx,
            &self.union(&run.mutants),
            &RunOptions {
                prebuilt: Some(prebuilt.clone()),
                ..self.options_for(None, 0, Share::Slot)
            },
        )?;
        if !ran.results.is_green() {
            return Ok(Validation::Failed {
                output: output(&ran),
                reason: why_red(&ran),
            });
        }
        Ok(Validation::Built {
            limits: vec![self.limit(elapsed); slots.len()],
            prebuilt: Some(prebuilt),
        })
    }

    /// Validates `source` in every one of `slots` in parallel, each building it.
    fn validate_each(&self, run: &FileRun<'_>, source: &str, slots: &[Slot]) -> Result<Validation> {
        let ran: Vec<Result<(TestRun, Duration)>> = std::thread::scope(|scope| {
            let handles: Vec<_> = slots
                .iter()
                .enumerate()
                .map(|(index, slot)| {
                    scope.spawn(move || {
                        let ctx = self.slot_ctx(slot)?;
                        write_file(slot, run, source)?;
                        self.build(index, &ctx, run)
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
        Ok(Validation::Built {
            limits,
            prebuilt: None,
        })
    }

    /// Runs every mutant of `run` across the slots, each slot taking the next mutant when it is
    /// free, gated mutants first; the outcomes in the order of `run.mutants`, `None` for each
    /// mutant the run's cancellation left without one (ADR-172).
    ///
    /// File copies under a runner that builds once run in the first slot alone, where the build
    /// is: any other slot would build everything to run one (ADR-150 §8). `finished` is called on
    /// this thread as each mutant finishes.
    pub fn run_file(
        &self,
        run: &FileRun<'_>,
        build: &Build<'_>,
        finished: &mut dyn FnMut(),
    ) -> Result<Vec<Option<Ran>>> {
        let mut order: Vec<usize> = (0..run.mutants.len()).collect();
        order.sort_by_key(|index| id(build, *index).is_none());
        let next = AtomicUsize::new(0);
        let found: Mutex<Vec<Option<Ran>>> = Mutex::new(vec![None; run.mutants.len()]);
        let reference = match build {
            Build::Copies => self.reference(run)?,
            Build::Schemata { .. } => Duration::ZERO,
        };
        let work = Work {
            run,
            build,
            reference,
            order: &order,
            next: &next,
            found: &found,
        };
        let failures: Vec<Error> = std::thread::scope(|scope| {
            let workers = match build {
                Build::Copies if self.runner.builds_once() => 1,
                _ => self.workers(run),
            };
            let (done, signals) = channel();
            let handles: Vec<_> = self.slots[..workers]
                .iter()
                .enumerate()
                .map(|(index, slot)| {
                    let (work, done) = (&work, done.clone());
                    scope.spawn(move || self.work(index, slot, work, &done))
                })
                .collect();
            drop(done);
            // Ends when every worker has ended and dropped its sender.
            for () in signals {
                finished();
            }
            handles
                .into_iter()
                .filter_map(|handle| handle.join().unwrap_or_else(|_| Err(panicked())).err())
                .collect()
        });
        if let Some(failure) = failures
            .into_iter()
            .find(|failure| *failure != Error::Cancelled)
        {
            return Err(failure);
        }
        Ok(found.into_inner().unwrap_or_default())
    }

    /// Writes `run`'s original file back into every slot it uses, after its mutants ended in an
    /// error (the budget, a cancellation, a baseline that is not green), so no slot keeps a
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

    /// One slot's work on a file: the runner's start-up for one test, then mutants until none is
    /// left, then the original file back. File copies are timed by the production's baseline,
    /// schemata by the validated build's. Each mutant it finishes is sent on `done`.
    fn work(&self, index: usize, slot: &Slot, work: &Work<'_>, done: &Sender<()>) -> Result<()> {
        let (run, build, reference) = (work.run, work.build, work.reference);
        let ctx = self.slot_ctx(slot)?;
        let (limit, holds, prebuilt) = match build {
            Build::Copies => {
                for (importer, text) in run.importers {
                    write(slot, importer, text)?;
                }
                (self.limit(reference), Holds::Original, None)
            }
            Build::Schemata {
                limits, prebuilt, ..
            } => (limits[index], Holds::Schemata, *prebuilt),
        };
        let startup = self.startup(slot, &ctx, run, prebuilt)?;
        let mut worker = Worker {
            index,
            slot,
            ctx,
            limit,
            reference,
            startup,
            holds,
        };
        while let Some(&mutant) = work.order.get(work.next.fetch_add(1, Ordering::SeqCst)) {
            let ran = self.one(&mut worker, work, mutant)?;
            if let Ok(mut found) = work.found.lock() {
                found[mutant] = Some(ran);
            }
            let _ = done.send(());
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
        if let (
            Some(id),
            Build::Schemata {
                source, prebuilt, ..
            },
        ) = (id(work.build, index), work.build)
        {
            if worker.holds != Holds::Schemata {
                write(slot, run.file, source.as_bytes())?;
                worker.holds = Holds::Schemata;
            }
            return self.mutant(worker, planned, id, *prebuilt);
        }
        write(slot, run.file, planned.source.as_bytes())?;
        let ran = self.mutant(worker, planned, 0, None);
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

    /// How long the production's unmutated baseline took, which times every file copy (ADR-185):
    /// this production's, else the store's under its suite hash, else measured now in the first
    /// slot and stored. The measurement runs the whole suite with `run`'s file holding one more
    /// trailing newline, which builds and loads like the original but is new to every build
    /// cache (by content or by time), so the run rebuilds what a mutant would. A baseline that is
    /// not green means the slot does not reproduce the tree's.
    fn reference(&self, run: &FileRun<'_>) -> Result<Duration> {
        if let Some(elapsed) = self.reference.elapsed() {
            return Ok(elapsed);
        }
        let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        let commands = self
            .runner
            .commands(self.ctx, self.whole)
            .unwrap_or_default();
        let key = self.reference.key(
            &self.ctx.ctx.root,
            &commands,
            self.runner.outputs(),
            threads(cores, self.slots.len()),
        );
        if let Some(elapsed) = key
            .as_deref()
            .and_then(|key| baseline::stored(&self.ctx.writer, key))
        {
            return Ok(self.reference.keep(elapsed));
        }
        let slot = &self.slots[0];
        let ctx = self.slot_ctx(slot)?;
        for (importer, text) in run.importers {
            write(slot, importer, text)?;
        }
        let mut renewed = run.original.to_vec();
        renewed.push(b'\n');
        write(slot, run.file, &renewed)?;
        let measured = self.unmutated(
            Unmutated {
                slot: 0,
                file: None,
                phase: MutationPhase::Build,
            },
            &ctx,
            self.whole,
            &self.options(None, 0),
        );
        write(slot, run.file, run.original)?;
        let (ran, elapsed) = measured?;
        // A run the cancellation reached measured nothing to keep.
        self.ctx.cancel.checkpoint()?;
        if !ran.results.is_green() {
            return Err(Error::Process {
                program: self.runner.id().to_string(),
                message: format!(
                    "the unmutated tree does not pass its tests in the mutation overlay at {}: {}",
                    slot.root().display(),
                    why_red(&ran)
                ),
            });
        }
        if let Some(key) = &key {
            baseline::store(&self.ctx.writer, key, elapsed);
        }
        Ok(self.reference.keep(elapsed))
    }

    /// The runner's start-up for a run of one test in `slot` (ADR-160 §3): the wall time of the
    /// fastest test `run`'s narrowed mutants select, run alone with no mutant (from `prebuilt`
    /// when the runner built once); `None` when per-test selection narrowed none of them. A run
    /// that is not green means the slot does not reproduce the baseline.
    fn startup(
        &self,
        slot: &Slot,
        ctx: &ProduceCtx,
        run: &FileRun<'_>,
        prebuilt: Option<&Prebuilt>,
    ) -> Result<Option<Duration>> {
        let ids: BTreeSet<&String> = run
            .mutants
            .iter()
            .filter_map(|planned| planned.tests.as_ref())
            .flatten()
            .collect();
        let ids: Vec<String> = ids.into_iter().cloned().collect();
        let Some(fastest) = sequence::order(&ids, self.baseline).into_iter().next() else {
            return Ok(None);
        };
        let mut options = self.options(None, 0);
        options.prebuilt = prebuilt.cloned();
        let selection = TestSelection::Tests(vec![fastest.id]);
        let started = Instant::now();
        let tmp = scratch()?;
        let ran = match self.runner.run(ctx, &selection, &within(&options, &tmp)) {
            Err(Error::OutOfMemory { limit, .. }) => return Err(over_budget(self.runner, limit)),
            ran => ran?,
        };
        let elapsed = started.elapsed();
        if !ran.results.is_green() {
            return Err(Error::Process {
                program: self.runner.id().to_string(),
                message: format!(
                    "the unmutated `{}` does not pass its fastest test alone in the mutation \
                     overlay at {}: {}",
                    run.file,
                    slot.root().display(),
                    why_red(&ran)
                ),
            });
        }
        Ok(Some(elapsed))
    }

    /// The schemata build every mutant of `run`'s file shares, in slot `index` (`ctx` is the
    /// slot's): the file's selected tests with no mutant, built where they run, recorded as the
    /// `build` phase.
    fn build(
        &self,
        index: usize,
        ctx: &ProduceCtx,
        run: &FileRun<'_>,
    ) -> Result<(TestRun, Duration)> {
        let options = self.options(None, 0);
        let selection = self.union(&run.mutants);
        self.unmutated(
            Unmutated {
                slot: index,
                file: Some(run.file),
                phase: MutationPhase::Build,
            },
            ctx,
            &selection,
            &options,
        )
    }

    /// Runs `selection` with no mutant, in the slot and for the file `run` names, with
    /// `options`, timed and recorded as `run`'s phase. A run over the slot's memory budget is an
    /// error naming the setting: every mutant would be.
    fn unmutated(
        &self,
        run: Unmutated<'_>,
        ctx: &ProduceCtx,
        selection: &TestSelection,
        options: &RunOptions,
    ) -> Result<(TestRun, Duration)> {
        let Unmutated {
            slot: index,
            file,
            phase,
        } = run;
        let started = Instant::now();
        let tmp = scratch()?;
        let ran = match self.runner.run(ctx, selection, &within(options, &tmp)) {
            Err(Error::OutOfMemory { limit, .. }) => return Err(over_budget(self.runner, limit)),
            ran => ran?,
        };
        let served = Served {
            file,
            slot: Some(index),
            tests: selected(selection),
        };
        self.phases.record(phase, served, started);
        Ok((ran, started.elapsed()))
    }

    /// A mutant's timeout after a baseline or validation run of `elapsed`.
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
                None => return self.unnarrowed(planned),
            }
        }
        TestSelection::Tests(ids.into_iter().collect())
    }

    /// What `planned` runs when per-test selection did not narrow it: its own root's suite
    /// when it has one (ADR-194), else the whole suite, less the slow tests left out of both.
    fn unnarrowed(&self, planned: &Planned) -> TestSelection {
        let Some(dir) = &planned.root else {
            return self.whole.clone();
        };
        let except = match self.whole {
            TestSelection::Except(ids) => ids.clone(),
            _ => Vec::new(),
        };
        TestSelection::Root {
            dir: dir.clone(),
            except,
        }
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
    /// `id`, from `prebuilt` when the runner built once): one at a time when per-test selection
    /// narrowed them, else together with the worker's timeout; the `run` phase.
    fn mutant(
        &self,
        worker: &Worker<'_>,
        planned: &Planned,
        id: u32,
        prebuilt: Option<&Prebuilt>,
    ) -> Result<Ran> {
        if let (Some(tests), Some(startup)) = (&planned.tests, worker.startup) {
            let sequence = Sequence {
                tests: sequence::order(tests, self.baseline),
                caps: Caps {
                    factor: self.settings.timeout_factor,
                    startup,
                    overall: self.settings.max_mutant_timeout,
                },
                id,
                prebuilt,
            };
            return self.sequence(worker, planned, &sequence);
        }
        let selection = planned
            .tests
            .clone()
            .map_or_else(|| self.unnarrowed(planned), TestSelection::Tests);
        let limit = worker.limit;
        let started = Instant::now();
        let strategy = if id == 0 {
            MutationStrategy::FileCopy
        } else {
            MutationStrategy::Schemata
        };
        let mut options = self.options(Some(limit), id);
        options.prebuilt = prebuilt.cloned();
        let tmp = scratch()?;
        let ran = self
            .runner
            .run(&worker.ctx, &selection, &within(&options, &tmp));
        let elapsed = started.elapsed();
        let served = Served {
            file: Some(planned.file()),
            slot: Some(worker.index),
            tests: selected(&selection),
        };
        self.phases.took(MutationPhase::Run, served, elapsed);
        // A run the cancellation reached has no outcome, whatever it reported (ADR-172).
        self.ctx.cancel.checkpoint()?;
        match ran {
            Err(Error::OutOfMemory { .. }) => Ok(out_of_memory(elapsed, strategy)),
            Err(error) => Err(error),
            Ok(ran) => Ok(classify(&ran, elapsed >= limit, elapsed, strategy)),
        }
    }
}

impl Execution<'_> {
    /// Runs `sequence`'s tests against the mutant in `worker`'s slot one at a time, fastest
    /// first, until one decides it (ADR-160 §6); the `run` phase.
    fn sequence(
        &self,
        worker: &Worker<'_>,
        planned: &Planned,
        sequence: &Sequence<'_>,
    ) -> Result<Ran> {
        let strategy = if sequence.id == 0 {
            MutationStrategy::FileCopy
        } else {
            MutationStrategy::Schemata
        };
        let started = Instant::now();
        let mut found = Found {
            outcome: MutantOutcome::Survived,
            tests: Vec::new(),
            killing_tests: Vec::new(),
            strategy,
            hung: None,
            slowest: Vec::new(),
        };
        self.decide(worker, sequence, &mut found, started)?;
        Ok(self.ran(worker, planned, found, started))
    }

    /// Runs `sequence`'s tests into `found` until one decides the mutant: a failure or a hang
    /// kills it, a run out of time for slowness leaves it a timeout, a run over its memory
    /// budget or one that did not build or load ends it; it survives when every test passes.
    fn decide(
        &self,
        worker: &Worker<'_>,
        sequence: &Sequence<'_>,
        found: &mut Found,
        started: Instant,
    ) -> Result<()> {
        for (position, test) in sequence.tests.iter().enumerate() {
            // A file copy's first run rebuilds the mutant, which the baseline run also did.
            let build = if position == 0 && found.strategy == MutationStrategy::FileCopy {
                worker.reference
            } else {
                Duration::ZERO
            };
            let limit = sequence.caps.limit(test, started.elapsed(), build);
            let Some(ran) = self.one_test(worker, test, limit.timeout, sequence)? else {
                found.outcome = MutantOutcome::OutOfMemory;
                return Ok(());
            };
            let run = found.add(ran);
            found.outcome = match sequence::verdict(&run, limit, test, &sequence.caps) {
                Verdict::Passed => continue,
                Verdict::Failed => MutantOutcome::Killed,
                Verdict::Hung => {
                    found.killing_tests = vec![test.id.clone()];
                    found.hung = Some(test.id.clone());
                    MutantOutcome::Killed
                }
                Verdict::Slow => {
                    found.slowest = sequence::slowest(&sequence.tests[position..]);
                    MutantOutcome::Timeout
                }
                Verdict::Unviable => MutantOutcome::Unviable,
            };
            return Ok(());
        }
        Ok(())
    }

    /// Runs `test` alone against `sequence`'s mutant with `timeout`; what it reported, or
    /// `None` when the run went over its memory budget.
    fn one_test(
        &self,
        worker: &Worker<'_>,
        test: &Test,
        timeout: Duration,
        sequence: &Sequence<'_>,
    ) -> Result<Option<(TestRun, bool)>> {
        let mut options = self.options(Some(timeout), sequence.id);
        options.prebuilt = sequence.prebuilt.cloned();
        let selection = TestSelection::Tests(vec![test.id.clone()]);
        let started = Instant::now();
        let tmp = scratch()?;
        let ran = self
            .runner
            .run(&worker.ctx, &selection, &within(&options, &tmp));
        // A run the cancellation reached has no outcome, whatever it reported (ADR-172).
        self.ctx.cancel.checkpoint()?;
        match ran {
            Err(Error::OutOfMemory { .. }) => Ok(None),
            Err(error) => Err(error),
            Ok(ran) => Ok(Some((ran, started.elapsed() >= timeout))),
        }
    }

    /// What `found` makes of `planned`'s run that began at `started`, recorded as its `run`
    /// phase over the tests per-test selection chose.
    fn ran(&self, worker: &Worker<'_>, planned: &Planned, found: Found, started: Instant) -> Ran {
        let elapsed = started.elapsed();
        let served = Served {
            file: Some(planned.file()),
            slot: Some(worker.index),
            tests: planned.tests.as_ref().map(Vec::len),
        };
        self.phases.took(MutationPhase::Run, served, elapsed);
        Ran {
            outcome: found.outcome,
            tests: found.tests,
            killing_tests: found.killing_tests,
            duration_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
            strategy: found.strategy,
            fallback: None,
            hung: found.hung,
            slowest: found.slowest,
        }
    }
}

/// A narrowed mutant's tests and how they run (ADR-160).
struct Sequence<'a> {
    /// In the order they run.
    tests: Vec<Test>,
    caps: Caps,
    /// The mutant's schemata id; 0 for a file copy.
    id: u32,
    prebuilt: Option<&'a Prebuilt>,
}

/// What a sequence has found so far.
struct Found {
    outcome: MutantOutcome,
    tests: Vec<String>,
    killing_tests: Vec<String>,
    strategy: MutationStrategy,
    hung: Option<String>,
    slowest: Vec<UnfinishedTest>,
}

impl Found {
    /// Adds what one test's run reported; what it says of that test.
    fn add(&mut self, (ran, reached): (TestRun, bool)) -> sequence::Run {
        let results = &ran.results;
        self.tests
            .extend(results.tests.iter().map(|test| test.id.clone()));
        self.killing_tests
            .extend(results.failures().map(|test| test.id.clone()));
        sequence::Run {
            failed: results.failures().next().is_some(),
            complete: results.incomplete().next().is_none(),
            reached,
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
        TestSelection::All | TestSelection::Except(_) | TestSelection::Root { .. } => None,
    }
}

/// One slot at work on a file: where it runs, its timeout, and what the file holds now.
struct Worker<'a> {
    /// The slot's index among the production's.
    index: usize,
    slot: &'a Slot,
    ctx: ProduceCtx,
    /// The timeout of a mutant that runs its selection as one run.
    limit: Duration,
    /// How long the production's baseline took, its build included; zero for a validated
    /// schemata build.
    reference: Duration,
    /// The runner's start-up for a run of one test; `None` when no mutant of the file is
    /// narrowed.
    startup: Option<Duration>,
    holds: Holds,
}

/// What the slots working on one file share.
struct Work<'a> {
    run: &'a FileRun<'a>,
    build: &'a Build<'a>,
    /// The production's baseline time for file copies; zero for schemata.
    reference: Duration,
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
        hung: None,
        slowest: Vec::new(),
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
        hung: None,
        slowest: Vec::new(),
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

/// The error of an unmutated run or build over its share of the memory budget, `limit`
/// bytes: every mutant of the file would be over it too.
fn over_budget(runner: &dyn Runner, limit: u64) -> Error {
    Error::Process {
        program: runner.id().to_string(),
        message: format!(
            "the tests of the unmutated file held more than {} of memory, one slot's share \
             of `max_mutant_memory`; raise it or lower `mutation.jobs`",
            size(limit)
        ),
    }
}

/// `bytes` in MiB, or in KiB (at least 1) when under one MiB, so a small share is never named as
/// zero.
fn size(bytes: u64) -> String {
    if bytes >= 1 << 20 {
        format!("{} MiB", bytes >> 20)
    } else {
        format!("{} KiB", (bytes >> 10).max(1))
    }
}

/// Writes `run`'s importers and `source`, its file's schemata, into `slot`.
fn write_file(slot: &Slot, run: &FileRun<'_>, source: &str) -> Result<()> {
    for (importer, text) in run.importers {
        write(slot, importer, text)?;
    }
    write(slot, run.file, source.as_bytes())
}

/// A temporary directory of one run's own, removed when it drops, so what the run's processes
/// leave in it (a run killed at its timeout or memory budget leaves whatever it made) does not
/// outlive the run. Its name is short: a test's socket under it must fit a socket's path.
fn scratch() -> Result<TempDir> {
    tempfile::Builder::new()
        .prefix("cm")
        .rand_bytes(4)
        .tempdir()
        .map_err(|error| {
            Error::Write(WriteError::Io {
                path: std::env::temp_dir(),
                message: error.to_string(),
            })
        })
}

/// `options` with `tmp` as the run's `TMPDIR`.
fn within(options: &RunOptions, tmp: &TempDir) -> RunOptions {
    let mut options = options.clone();
    options
        .env
        .push(("TMPDIR".to_owned(), tmp.path().display().to_string()));
    options
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

/// What an unmutated run serves: the slot it runs in, the file whose mutants it times or
/// validates (`None`: the production's), and the phase it is recorded as.
#[derive(Clone, Copy)]
struct Unmutated<'a> {
    slot: usize,
    file: Option<&'a str>,
    phase: MutationPhase,
}

#[cfg(test)]
mod tests {
    use super::size;

    #[test]
    fn a_share_under_a_mebibyte_is_named_in_kibibytes_and_never_as_zero() {
        assert_eq!(
            (size(256 << 10), size(1 << 20), size(3 << 30), size(1)),
            (
                "256 KiB".to_owned(),
                "1 MiB".to_owned(),
                "3072 MiB".to_owned(),
                "1 KiB".to_owned()
            )
        );
    }
}
