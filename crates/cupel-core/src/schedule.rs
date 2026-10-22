// ABOUTME: Scheduler v1 (design §8.9, ADR-041): resolve the symbols in scope, split selected checks into
// ABOUTME: supported (check, language) tasks and skips, produce each needed fact once, run, and combine.
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::num::NonZeroUsize;
use std::time::Instant;

use crate::base::BaseTree;
use crate::budget::{Resources, TaskDuration, TaskShape};
use crate::cancel::Cancellation;
use crate::check::{Check, CheckCtx, CheckRegistry, Manifest, Plan};
use crate::config::Config;
use crate::dry_run::{PlannedCheck, PlannedSkip, RunPlan};
use crate::error::{ArgumentError, Error, Result};
use crate::event::{Event, EventSink};
use crate::fact::{FactKind, SharedFact};
use crate::facts::Facts;
use crate::finding::{Finding, Severity};
use crate::grade::CostClass;
use crate::id::{LangId, RunId};
use crate::joint::Deciding;
use crate::lang::Lang;
use crate::matching::{self, SymbolMatch};
use crate::matrix::{Support, SupportMatrix};
use crate::mutation::MutationOutcomes;
use crate::oracle::RunOracle;
use crate::producer::{
    Availability, ChangedLines, Ctx, Focus, ProduceCtx, ProducerRegistry, Upstream,
};
use crate::rank::{Measured, Measurement};
use crate::result::{CheckResult, CheckStatus, SkipReason, Skipped};
use crate::scope::{Scope, SymbolSelector, select_symbols, touched};
use crate::syntax::{NormTree, Symbols};
use crate::test_results::{TestResults, Unevaluable};
use crate::writer::Writer;

mod budgeted;

/// The registries a run draws on.
pub struct Scheduler<'a> {
    pub checks: &'a CheckRegistry,
    pub producers: &'a ProducerRegistry,
}

/// Everything one run of the scheduler needs.
pub struct Job<'a> {
    pub run: RunId,
    /// The run's writer; producers get it confined to `.cupel/`, checks a scratch directory.
    pub writer: &'a Writer,
    pub cancel: Cancellation,
    pub config: &'a Config,
    /// The selected checks, in registry order.
    pub selected: Vec<&'a dyn Check>,
    /// The languages that own at least one file in scope, in ID order.
    pub langs: Vec<Lang>,
    /// The files in scope (the walk, already limited by paths and range).
    pub files: Vec<String>,
    pub selectors: Vec<SymbolSelector>,
    /// Whether a selector that matches no symbol is a usage error (no range was given).
    pub selectors_must_match: bool,
    /// The base side of a range scope; `None` without a range (ADR-056).
    pub base: Option<BaseTree>,
    /// The run's seed, handed to every producer (ADR-078).
    pub seed: u64,
    /// The run's depth, handed to every producer (ADR-080).
    pub depth: CostClass,
    /// The lines a range scope changed, per file; `None` without a range (ADR-080).
    pub changed: Option<ChangedLines>,
    /// The run's budget, concurrency cap, and the store's history of durations (ADR-086).
    pub resources: Resources,
    /// The oracle the run's checks compare against, handed to every producer (ADR-093 §2).
    pub oracle: Option<RunOracle>,
    /// `--cases`, handed to every producer (ADR-094 §5, ADR-098).
    pub cases: Option<u64>,
}

/// What a run (or a dry run's planning) produced.
#[derive(Debug, Default)]
pub struct Outcome {
    /// One entry per selected check, its languages combined (ADR-064).
    pub checks: Vec<CheckResult>,
    /// Every check's findings plus producers' findings, de-duplicated by ID, sorted.
    pub findings: Vec<Finding>,
    /// Every per-target value the checks measured, for `survey`'s rankings (ADR-062).
    pub measurements: Vec<Measured>,
    /// Symbols in scope across languages.
    pub symbols: u64,
    /// Cancellation cut work short; the envelope is `partial`.
    pub interrupted: bool,
    /// What a dry run would do; `None` for a real run.
    pub plan: Option<RunPlan>,
    /// Every test run the run made (the baseline), by language, for the store's index.
    pub test_results: Vec<(LangId, TestResults)>,
    /// Every mutation outcome the run produced, by language, for the store's index (ADR-080).
    pub mutation_outcomes: Vec<(LangId, MutationOutcomes)>,
    /// The red baseline that refused the run: no check ran, so `checks` is empty (ADR-076).
    pub refused: Option<Unevaluable>,
    /// How long each production and check run took, for the store's history (ADR-086 §2).
    pub durations: Vec<TaskDuration>,
    /// Under a range, the symbols the range removed matched to those it added, per language
    /// (ADR-131); empty without a range.
    pub matches: Vec<SymbolMatch>,
}

/// The scheduler's fact store opened for a verb that reads facts itself rather than running
/// checks (`tests --impacted`, ADR-077 §4): facts are produced exactly as a run produces them.
pub struct Production<'a>(FactStore<'a>);

impl<'a> Scheduler<'a> {
    /// A fact store over `job`'s files and configuration.
    pub fn production(&self, job: &Job<'a>) -> Production<'a> {
        Production(FactStore::new(self.producers, job))
    }
}

impl Production<'_> {
    /// `kind` for `lang`, produced on first request (with the producer's needs) and kept.
    pub fn ensure(
        &mut self,
        kind: FactKind,
        lang: &Lang,
        sink: &mut dyn EventSink,
    ) -> Result<SharedFact> {
        self.0.ensure(kind, lang, sink)
    }

    /// Whether the run's cancellation cut production short.
    pub fn interrupted(&self) -> bool {
        self.0.interrupted
    }

    /// The producers' findings, each once, sorted.
    pub fn into_findings(self) -> Vec<Finding> {
        let mut findings = self.0.findings;
        sort_findings(&mut findings);
        findings
    }
}

/// One (check, language) the support matrix allows, with its plan.
struct Task<'a> {
    check: &'a dyn Check,
    lang: Lang,
    /// The family's other supported languages, for a family-wide check (ADR-051).
    siblings: Vec<Lang>,
    scope: Scope,
    plan: Plan,
    /// What running it is estimated to cost: its own cost plus the producer runs it brings in,
    /// in the order the run takes tasks (ADR-086 §10).
    cost_ms: u64,
}

/// A (check, language) that will not run, and why.
struct Skip<'a> {
    check: &'a dyn Check,
    skipped: Skipped,
}

impl Scheduler<'_> {
    /// Runs `job`, streaming events to `sink`.
    pub fn run(&self, job: &Job<'_>, sink: &mut dyn EventSink) -> Result<Outcome> {
        let mut store = FactStore::new(self.producers, job);
        let base = job
            .base
            .as_ref()
            .map(|base| FactStore::at(self.producers, job, base));
        let mut prepared = self.prepare(job, &mut store, sink)?;
        budgeted::schedule(job, &mut store, &mut prepared);
        prepared.announce(job, sink);
        let mut execution = Execution::start(job, store, base, &prepared);
        if let Some(red) = execution.baseline(self.producers, &prepared.tasks, sink) {
            return Ok(Outcome {
                findings: execution.store.findings,
                symbols: prepared.symbols,
                test_results: execution.store.test_results,
                mutation_outcomes: execution.store.mutation_outcomes,
                refused: Some(red),
                durations: execution.store.durations,
                ..Outcome::default()
            });
        }
        let (checks, mut findings, measurements) = execution.run_checks(&prepared, sink);
        let matches = execution.matches();
        let Execution {
            store,
            interrupted,
            mut durations,
            ..
        } = execution;
        durations.extend(store.durations);
        // P2.2 persists this to the store.
        findings.extend(store.findings);
        sort_findings(&mut findings);
        Ok(Outcome {
            checks,
            findings,
            measurements,
            symbols: prepared.symbols,
            interrupted: interrupted || store.interrupted,
            plan: None,
            test_results: store.test_results,
            mutation_outcomes: store.mutation_outcomes,
            refused: None,
            durations,
            matches,
        })
    }

    /// Plans `job` without running a check: what would run, with targets and estimates, and
    /// what would be skipped. Resolving the scope parses files but writes nothing.
    pub fn plan(&self, job: &Job<'_>, sink: &mut dyn EventSink) -> Result<Outcome> {
        let mut store = FactStore::new(self.producers, job);
        let mut prepared = self.prepare(job, &mut store, sink)?;
        budgeted::schedule(job, &mut store, &mut prepared);
        let would_run = job
            .selected
            .iter()
            .filter_map(|check| {
                let tasks: Vec<&Task<'_>> = prepared
                    .tasks
                    .iter()
                    .filter(|task| same(task.check, *check))
                    .collect();
                if tasks.is_empty() && !job.langs.is_empty() {
                    return None;
                }
                let manifest = check.manifest();
                let langs: Vec<&Lang> = tasks.iter().map(|task| &task.lang).collect();
                Some(PlannedCheck {
                    id: manifest.id.clone(),
                    targets: tasks.iter().map(|task| task.plan.targets).sum(),
                    est_ms: tasks.iter().map(|task| task.cost_ms).sum(),
                    needs: self.planned_needs(manifest, &langs, &store),
                    note: None,
                })
            })
            .collect();
        let would_skip = prepared
            .skips
            .iter()
            .map(|skip| PlannedSkip {
                id: skip.check.manifest().id.clone(),
                skipped: skip.skipped.clone(),
            })
            .collect();
        let mut findings = store.findings;
        sort_findings(&mut findings);
        Ok(Outcome {
            checks: Vec::new(),
            findings,
            measurements: Vec::new(),
            symbols: prepared.symbols,
            interrupted: store.interrupted,
            plan: Some(RunPlan {
                would_run,
                would_skip,
                ..RunPlan::default()
            }),
            test_results: Vec::new(),
            mutation_outcomes: Vec::new(),
            refused: None,
            durations: Vec::new(),
            matches: Vec::new(),
        })
    }

    /// A planned check's needs and wants by name; then, per language, how the suite facts it
    /// reaches would be made, in one joint run or each on its own (ADR-084 §8); then each want of
    /// its needs' producers that the run would not simply make (ADR-083): with the producer's
    /// note, such as `per-test-map(cached)`, or with its cost when that is beyond the run's
    /// depth, such as `per-test-map(deep)`, so it would be left out.
    fn planned_needs(
        &self,
        manifest: &Manifest,
        langs: &[&Lang],
        store: &FactStore<'_>,
    ) -> Vec<String> {
        let mut needs: Vec<String> = manifest
            .needs
            .iter()
            .chain(&manifest.wants)
            .map(ToString::to_string)
            .collect();
        let mut covered = BTreeSet::new();
        for lang in langs {
            let deciding = store.deciding(lang);
            let reached = deciding.reached(&manifest_kinds(manifest));
            let (entries, joint) = deciding.planned(&reached);
            covered.extend(joint);
            for entry in entries {
                if !needs.contains(&entry) {
                    needs.push(entry);
                }
            }
        }
        for entry in self.wants_planned(manifest, langs, store, &covered) {
            if !needs.contains(&entry) {
                needs.push(entry);
            }
        }
        needs
    }

    /// Each want of `manifest`'s needs' producers that the run would not simply make and no
    /// joint run covers (ADR-083): `per-test-map(cached)`, or with its cost when that is beyond
    /// the run's depth, `per-test-map(deep)`.
    fn wants_planned(
        &self,
        manifest: &Manifest,
        langs: &[&Lang],
        store: &FactStore<'_>,
        covered: &BTreeSet<FactKind>,
    ) -> Vec<String> {
        let mut entries = Vec::new();
        for (kind, lang) in manifest
            .needs
            .iter()
            .flat_map(|kind| langs.iter().map(move |lang| (kind, *lang)))
        {
            let wants = self
                .producers
                .producers_for(*kind, lang)
                .next()
                .map_or(&[][..], |producer| producer.wants());
            for want in wants.iter().filter(|want| !covered.contains(*want)) {
                let upstream = store.upstream(store.made_kinds(lang).iter(), lang);
                let Some(estimate) = self
                    .producers
                    .producers_for(*want, lang)
                    .next()
                    .map(|producer| producer.estimate(&store.root, &upstream))
                else {
                    continue;
                };
                let how = if estimate.cost > store.depth {
                    Some(estimate.cost.to_string())
                } else {
                    estimate.note
                };
                let entry = how.map(|how| format!("{want}({how})"));
                if let Some(entry) = entry.filter(|entry| !entries.contains(entry)) {
                    entries.push(entry);
                }
            }
        }
        entries
    }

    /// Resolves the symbols in scope, then splits the selected checks into tasks and skips.
    fn prepare<'a>(
        &self,
        job: &'a Job<'a>,
        store: &mut FactStore<'_>,
        sink: &mut dyn EventSink,
    ) -> Result<Prepared<'a>> {
        let scopes = resolve_symbols(job, store, sink)?;
        if !job.selectors.is_empty() {
            store.selected = scopes
                .iter()
                .map(|(lang, scope)| (lang.clone(), scope.symbols.clone().unwrap_or_default()))
                .collect();
        }
        let symbols = scopes
            .values()
            .map(|scope| scope.symbols.as_ref().map_or(0, BTreeSet::len) as u64)
            .sum();
        let matrix = SupportMatrix::compute(
            self.checks,
            self.producers,
            &job.langs,
            &Ctx {
                root: job.writer.root().to_path_buf(),
                exclude: job.config.file.scope.exclude.clone(),
                lang: job.config.file.lang.clone(),
            },
        );
        let mut prepared = Prepared {
            tasks: Vec::new(),
            skips: Vec::new(),
            symbols,
        };
        for check in &job.selected {
            prepared.add(job, *check, &matrix, &scopes)?;
        }
        Ok(prepared)
    }
}

/// A real run in progress: the fact stores and whether the timeout cut work short.
struct Execution<'a, 'j> {
    job: &'j Job<'j>,
    store: FactStore<'a>,
    /// The base side of a range scope (ADR-056).
    base: Option<FactStore<'a>>,
    /// Some task was cut short or skipped by the timeout or the budget.
    interrupted: bool,
    /// The run's cancellation, with the budget's deadline when there is one (ADR-086 §6).
    cancel: Cancellation,
    /// How long each check run took.
    durations: Vec<TaskDuration>,
}

impl<'a, 'j> Execution<'a, 'j> {
    /// The run of `prepared` over `store` (and a range's `base`): every check and production
    /// runs under the run's cancellation, which the budget's deadline also fires (ADR-086 §6).
    fn start(
        job: &'j Job<'j>,
        mut store: FactStore<'a>,
        mut base: Option<FactStore<'a>>,
        prepared: &Prepared<'_>,
    ) -> Self {
        let cancel = budgeted::cancellation(job);
        store.cancel = cancel.clone();
        if let Some(base) = &mut base {
            base.cancel = cancel.clone();
        }
        Self {
            job,
            store,
            base,
            interrupted: prepared.over_budget(),
            cancel,
            durations: Vec::new(),
        }
    }

    /// Runs every selected check in registry order: each check's one entry, every check's
    /// findings, and every measurement.
    fn run_checks(
        &mut self,
        prepared: &Prepared<'_>,
        sink: &mut dyn EventSink,
    ) -> (Vec<CheckResult>, Vec<Finding>, Vec<Measured>) {
        let mut checks = Vec::new();
        let mut findings = Vec::new();
        let mut measurements = Vec::new();
        for check in prepared.running_order(&self.job.selected) {
            let (tasks, skips) = prepared.of(check);
            // A check runs when some language supports it, or when no language is in scope.
            if tasks.is_empty() && !skips.is_empty() {
                checks.push(skipped_entry(check, skips));
                continue;
            }
            let finished = self.run_check(check, &tasks, skips, sink);
            checks.push(finished.result);
            findings.extend(finished.findings);
            measurements.extend(finished.measurements);
        }
        let position = |result: &CheckResult| {
            self.job
                .selected
                .iter()
                .position(|check| check.manifest().id == result.id)
        };
        checks.sort_by_key(position);
        (checks, findings, measurements)
    }

    /// Under a range, each language's matches between the symbols the range removed and those
    /// it added (ADR-131), from both sides' `Symbols` and `NormTree`; none for a language where
    /// a side lacks either. Base-side producer findings describe replaced code: dropped.
    fn matches(&mut self) -> Vec<SymbolMatch> {
        let Some(base) = &mut self.base else {
            return Vec::new();
        };
        let mut matches = Vec::new();
        for lang in &self.job.langs {
            // The head side first: a language without trees there needs nothing from the base.
            let Some(after) = symbol_trees(&mut self.store, lang, &mut Vec::new()) else {
                continue;
            };
            let Some(before) = symbol_trees(base, lang, &mut Vec::new()) else {
                continue;
            };
            if let (Some(before), Some(after)) = (candidates(&before), candidates(&after)) {
                matches.extend(matching::across(&before, &after));
            }
        }
        matches
    }

    /// The baseline test run (ADR-076 §5): `TestResults` for every language a task's needs reach
    /// it in, before any check runs. The first red suite refuses the run; any other failure is
    /// left to the checks that need the fact.
    fn baseline(
        &mut self,
        producers: &ProducerRegistry,
        tasks: &[Task<'_>],
        sink: &mut dyn EventSink,
    ) -> Option<Unevaluable> {
        let mut langs: Vec<&Lang> = Vec::new();
        for task in tasks {
            let needs = &task.check.manifest().needs;
            for lang in std::iter::once(&task.lang).chain(&task.siblings) {
                let seen = langs.iter().any(|seen| seen.id == lang.id);
                if !seen && reaches(producers, needs, lang, &mut Vec::new()) {
                    langs.push(lang);
                }
            }
        }
        for lang in langs {
            let _ = self.store.ensure(FactKind::TestResults, lang, sink);
            if self.store.refused.is_some() {
                return self.store.refused.take();
            }
        }
        None
    }

    /// Runs every task of `check` and combines them with the `skips` into its one entry.
    fn run_check(
        &mut self,
        check: &dyn Check,
        tasks: &[&Task<'_>],
        skips: Vec<Skipped>,
        sink: &mut dyn EventSink,
    ) -> Finished {
        let mut run = CheckRun::start(check, tasks, sink);
        for task in tasks {
            let result = self.run_task(task, &mut run.sink(&task.lang, sink));
            // A check may skip itself for its own reasons (ADR-048); only a cut-short check or
            // a timeout skip makes the run partial.
            self.interrupted |= result.status == CheckStatus::Partial
                || result.skipped.as_ref().is_some_and(|skipped| {
                    matches!(skipped.reason, SkipReason::Timeout | SkipReason::Budget)
                });
            run.record(task, result, sink);
        }
        run.finish(skips, sink)
    }

    /// Produces what `task` reads and runs it; never fails, since one check's failure must not
    /// stop the others (ADR-041).
    fn run_task(&mut self, task: &Task<'_>, sink: &mut CheckSink<'_>) -> CheckResult {
        let started = Instant::now();
        let ended = |status, skipped| CheckResult {
            duration_ms: elapsed_ms(started),
            ..unfinished(task.check.manifest(), status, skipped)
        };
        if self.job.cancel.is_cancelled() {
            return ended(
                CheckStatus::Skipped,
                Some(skip_reason(SkipReason::Timeout, &task.lang.id)),
            );
        }
        if let Some(est_ms) = self.over_budget(task) {
            sink.forward.emit(Event::Skip {
                id: task.check.manifest().id.clone(),
                reason: SkipReason::Budget,
            });
            return ended(
                CheckStatus::Skipped,
                Some(budgeted::skip(&task.lang.id, est_ms)),
            );
        }
        if self.store.ran_no_test(&task.lang.id)
            && reaches(
                self.store.producers,
                &task.check.manifest().needs,
                &task.lang,
                &mut Vec::new(),
            )
        {
            return ended(CheckStatus::Skipped, Some(no_tests_ran(&task.lang.id)));
        }
        let ran = match self.facts_for(task, &mut *sink.forward) {
            Ok(facts) => self.timed_run(task, &facts, sink),
            Err(error) => Err(error),
        };
        match ran {
            Ok(mut ran) => {
                ran.duration_ms = elapsed_ms(started);
                ran
            }
            Err(error) => {
                // What failed, so the run says why without a rerun; a timeout says so already.
                let mut result = ended(status_of(&error), None);
                if error != Error::Cancelled {
                    result.notes.push(error.to_string());
                }
                result
            }
        }
    }

    /// Runs `task`'s check over `facts` under the run's cancellation, timing a run that finished
    /// for the store's history (ADR-086 §2).
    fn timed_run(
        &mut self,
        task: &Task<'_>,
        facts: &Facts,
        sink: &mut CheckSink<'_>,
    ) -> Result<CheckResult> {
        let mut ctx = check_ctx(self.job, task.check, &task.lang.id, task.scope.clone());
        ctx.cancel = self.cancel.clone();
        let started = Instant::now();
        let ran = task.check.run(&ctx, facts, &task.plan, sink);
        if ran
            .as_ref()
            .is_ok_and(|ran| matches!(ran.status, CheckStatus::Pass | CheckStatus::Fail))
        {
            self.durations.push(TaskDuration {
                shape: TaskShape::check(
                    &task.check.manifest().id,
                    &task.lang.id,
                    task.plan.targets,
                ),
                duration_ms: elapsed_ms(started),
            });
        }
        ran
    }

    /// The facts `task`'s check reads: its language's, its family siblings', the range, and the
    /// base side's.
    fn facts_for(&mut self, task: &Task<'_>, forward: &mut dyn EventSink) -> Result<Facts> {
        let manifest = task.check.manifest();
        let mut facts = Facts::for_manifest(manifest);
        serve(
            &mut self.store,
            manifest,
            &task.lang,
            forward,
            &mut |fact| facts.insert_shared(fact),
        )?;
        for sibling in &task.siblings {
            serve(&mut self.store, manifest, sibling, forward, &mut |fact| {
                facts.insert_shared_sibling(fact)
            })?;
        }
        if let Some(tree) = &self.job.base {
            facts.set_range(tree.range.clone());
        }
        if let Some(base) = &mut self.base {
            for kind in task.check.base_needs() {
                // Base-side producer findings describe code the change replaced: dropped.
                let fact = base.ensure(*kind, &task.lang, &mut Vec::new())?;
                facts.insert_base_shared(&fact)?;
            }
        }
        Ok(facts)
    }
}

/// `lang`'s `Symbols` and `NormTree` from `store`, when it can make both.
fn symbol_trees(
    store: &mut FactStore<'_>,
    lang: &Lang,
    sink: &mut dyn EventSink,
) -> Option<(SharedFact, SharedFact)> {
    let symbols = store.ensure(FactKind::Symbols, lang, sink).ok()?;
    let trees = store.ensure(FactKind::NormTree, lang, sink).ok()?;
    Some((symbols, trees))
}

/// The symbols of a `(Symbols, NormTree)` pair, joined for matching.
fn candidates((symbols, trees): &(SharedFact, SharedFact)) -> Option<Vec<matching::Candidate<'_>>> {
    Some(matching::candidates(
        symbols.downcast_ref::<Symbols>()?,
        trees.downcast_ref::<NormTree>()?,
    ))
}

/// Whether producing `needs` for `lang` reaches `TestResults`, through any producer's needs.
/// `visiting` holds the kinds above this call, to cut dependency cycles.
fn reaches(
    producers: &ProducerRegistry,
    needs: &[FactKind],
    lang: &Lang,
    visiting: &mut Vec<FactKind>,
) -> bool {
    needs.iter().any(|kind| {
        if *kind == FactKind::TestResults {
            return true;
        }
        if visiting.contains(kind) {
            return false;
        }
        visiting.push(*kind);
        let found = producers
            .producers_for(*kind, lang)
            .any(|producer| reaches(producers, producer.needs(), lang, visiting));
        visiting.pop();
        found
    })
}

/// A check's needs, then its wants, which a run makes whatever they cost.
fn manifest_kinds(manifest: &Manifest) -> Vec<FactKind> {
    manifest
        .needs
        .iter()
        .chain(&manifest.wants)
        .copied()
        .collect()
}

/// Produces `manifest`'s needs for `lang`, failing on the first that cannot be made, then
/// whichever of its wants can be, handing each to `insert`.
fn serve(
    store: &mut FactStore<'_>,
    manifest: &Manifest,
    lang: &Lang,
    forward: &mut dyn EventSink,
    insert: &mut dyn FnMut(&SharedFact) -> Result<()>,
) -> Result<()> {
    for kind in &manifest.needs {
        insert(&store.ensure(*kind, lang, forward)?)?;
    }
    for kind in &manifest.wants {
        if let Ok(fact) = store.ensure(*kind, lang, forward) {
            let _ = insert(&fact);
        }
    }
    Ok(())
}

/// A result of `manifest`'s check with `status`, before it ran or when it could not run.
fn unfinished(manifest: &Manifest, status: CheckStatus, skipped: Option<Skipped>) -> CheckResult {
    CheckResult {
        id: manifest.id.clone(),
        status,
        tier: manifest.tier,
        cost: manifest.cost,
        score: None,
        metrics: BTreeMap::new(),
        delta: None,
        findings: 0,
        duration_ms: 0,
        skipped,
        oracle: None,
        notes: Vec::new(),
    }
}

/// The languages each task of `check` covers: one per language, or one group per family for a
/// family-wide check (ADR-051), each in language-ID order as `languages` is.
fn task_groups(check: &dyn Check, languages: Vec<Lang>) -> Vec<Vec<Lang>> {
    if !check.spans_family() {
        return languages.into_iter().map(|lang| vec![lang]).collect();
    }
    let mut groups: Vec<Vec<Lang>> = Vec::new();
    for lang in languages {
        match groups
            .iter_mut()
            .find(|group| group[0].family == lang.family)
        {
            Some(group) => group.push(lang),
            None => groups.push(vec![lang]),
        }
    }
    groups
}

/// `scope` widened by `other`'s symbols; both cover the same files.
fn union(mut scope: Scope, other: Option<&Scope>) -> Scope {
    if let (Some(keys), Some(more)) = (
        scope.symbols.as_mut(),
        other.and_then(|other| other.symbols.as_ref()),
    ) {
        keys.extend(more.iter().cloned());
    }
    scope
}

struct Prepared<'a> {
    tasks: Vec<Task<'a>>,
    skips: Vec<Skip<'a>>,
    symbols: u64,
}

impl<'a> Prepared<'a> {
    /// `check`'s tasks, and the skip of every (check, language) the matrix or a missing range
    /// rules out.
    fn add(
        &mut self,
        job: &Job<'a>,
        check: &'a dyn Check,
        matrix: &SupportMatrix,
        scopes: &HashMap<LangId, Scope>,
    ) -> Result<()> {
        let manifest = check.manifest();
        let mut supported = Vec::new();
        for lang in &job.langs {
            match matrix
                .get(&manifest.id, &lang.id)
                .cloned()
                .unwrap_or(Support::Supported)
            {
                Support::Supported => supported.push(lang.clone()),
                support => self.skips.push(Skip {
                    check,
                    skipped: unsupported(&support, &lang.id),
                }),
            }
        }
        for mut langs in task_groups(check, supported) {
            let lang = langs.remove(0);
            if !check.base_needs().is_empty() && job.base.is_none() {
                self.skips.push(Skip {
                    check,
                    skipped: needs_range(&manifest.id, &lang.id),
                });
                continue;
            }
            let scope = langs.iter().fold(
                scopes.get(&lang.id).cloned().unwrap_or_default(),
                |scope, sibling| union(scope, scopes.get(&sibling.id)),
            );
            let ctx = check_ctx(job, check, &lang.id, scope.clone());
            let plan = check.plan(&ctx, &scope)?;
            self.tasks.push(Task {
                check,
                lang,
                siblings: langs,
                scope,
                cost_ms: plan.est_ms,
                plan,
            });
        }
        Ok(())
    }

    /// The checks of `selected` in the order their first task runs, then those with none, in
    /// registry order: the registry's order without a budget, the knapsack's with one.
    fn running_order(&self, selected: &[&'a dyn Check]) -> Vec<&'a dyn Check> {
        let first = |check: &dyn Check| {
            self.tasks
                .iter()
                .position(|task| same(task.check, check))
                .unwrap_or(usize::MAX)
        };
        let mut order = selected.to_vec();
        order.sort_by_key(|check| first(*check));
        order
    }

    /// Emits the run's `plan` event, with what the tasks taken are estimated to cost, and a
    /// `skip` event per (check, language) left out.
    fn announce(&self, job: &Job<'_>, sink: &mut dyn EventSink) {
        sink.emit(Event::Plan {
            run: job.run,
            checks: u32::try_from(job.selected.len()).unwrap_or(u32::MAX),
            est_ms: self.tasks.iter().map(|task| task.cost_ms).sum(),
        });
        for skip in &self.skips {
            sink.emit(Event::Skip {
                id: skip.check.manifest().id.clone(),
                reason: skip.skipped.reason,
            });
        }
    }

    /// Whether the budget skipped any task before the run.
    fn over_budget(&self) -> bool {
        self.skips
            .iter()
            .any(|skip| skip.skipped.reason == SkipReason::Budget)
    }

    /// `check`'s tasks and the skips of its ruled-out languages.
    fn of(&self, check: &dyn Check) -> (Vec<&Task<'a>>, Vec<Skipped>) {
        let tasks = self
            .tasks
            .iter()
            .filter(|task| same(task.check, check))
            .collect();
        let skips = self
            .skips
            .iter()
            .filter(|skip| same(skip.check, check))
            .map(|skip| skip.skipped.clone())
            .collect();
        (tasks, skips)
    }
}

/// The symbols in scope per language: every symbol of its files, or those the selectors match,
/// and under a range only those its changed lines touch (ADR-142).
fn resolve_symbols(
    job: &Job<'_>,
    store: &mut FactStore<'_>,
    sink: &mut dyn EventSink,
) -> Result<HashMap<LangId, Scope>> {
    let mut matched = vec![false; job.selectors.len()];
    let mut scopes = HashMap::new();
    for lang in &job.langs {
        let keys = symbols_in_scope(job, store, lang, &mut matched, sink).unwrap_or_default();
        let keys = within_range(job, store, lang, keys, sink);
        scopes.insert(lang.id.clone(), scope_of(job, keys));
    }
    if job.selectors_must_match
        && !store.interrupted
        && let Some(index) = matched.iter().position(|hit| !hit)
    {
        let text = selector_text(&job.selectors[index]);
        return Err(ArgumentError::new(
            "--symbols",
            Some(&text),
            format!("`--symbols {text}` matches no symbol in scope"),
            "Pass a display name as findings print it (`module::Type::method`; `.` and `::` \
             are interchangeable) or a structural hash (`sh_…`)."
                .to_owned(),
        )
        .into());
    }
    Ok(scopes)
}

/// The keys of `lang`'s symbols in scope, marking in `matched` the selectors that matched one;
/// `None` when the language's symbols cannot be produced.
fn symbols_in_scope(
    job: &Job<'_>,
    store: &mut FactStore<'_>,
    lang: &Lang,
    matched: &mut [bool],
    sink: &mut dyn EventSink,
) -> Option<BTreeSet<crate::syntax::SymbolKey>> {
    let produced = store.ensure(FactKind::Symbols, lang, sink).ok()?;
    let symbols = produced.downcast_ref::<Symbols>()?;
    if job.selectors.is_empty() {
        return Some(
            symbols
                .symbols
                .iter()
                .map(crate::syntax::SymbolDef::key)
                .collect(),
        );
    }
    let norm = job
        .selectors
        .iter()
        .any(SymbolSelector::needs_hashes)
        .then(|| store.ensure(FactKind::NormTree, lang, sink).ok())
        .flatten();
    let norm = norm
        .as_ref()
        .and_then(|fact| fact.downcast_ref::<NormTree>());
    let (keys, hits) = select_symbols(&job.selectors, symbols, norm);
    for (seen, hit) in matched.iter_mut().zip(hits) {
        *seen |= hit;
    }
    Some(keys)
}

/// Under a range, `keys` less the symbols of `lang` its changed lines do not touch (ADR-142);
/// `keys` as they are without a range.
fn within_range(
    job: &Job<'_>,
    store: &mut FactStore<'_>,
    lang: &Lang,
    mut keys: BTreeSet<crate::syntax::SymbolKey>,
    sink: &mut dyn EventSink,
) -> BTreeSet<crate::syntax::SymbolKey> {
    let Some(changed) = &job.changed else {
        return keys;
    };
    let Ok(produced) = store.ensure(FactKind::Symbols, lang, sink) else {
        return keys;
    };
    if let Some(symbols) = produced.downcast_ref::<Symbols>() {
        let untouched: BTreeSet<_> = symbols
            .symbols
            .iter()
            .filter(|symbol| !touched(changed, symbol))
            .map(crate::syntax::SymbolDef::key)
            .collect();
        keys.retain(|key| !untouched.contains(key));
    }
    keys
}

fn scope_of(job: &Job<'_>, keys: BTreeSet<crate::syntax::SymbolKey>) -> Scope {
    Scope {
        files: job.files.clone(),
        symbols: Some(keys),
    }
}

fn selector_text(selector: &SymbolSelector) -> String {
    match selector {
        SymbolSelector::Hash(hash) => hash.to_string(),
        SymbolSelector::Name(name) => name.clone(),
    }
}

fn check_ctx(job: &Job<'_>, check: &dyn Check, lang: &LangId, scope: Scope) -> CheckCtx {
    CheckCtx {
        scope,
        config: job.config.check_config(&check.manifest().id, lang),
        writer: job.writer.scratch(&job.run),
        cancel: job.cancel.clone(),
    }
}

/// Facts produced so far in this run, each at most once per (kind, language).
struct FactStore<'a> {
    producers: &'a ProducerRegistry,
    root: Ctx,
    writer: Writer,
    cancel: Cancellation,
    config: &'a Config,
    seed: u64,
    depth: CostClass,
    changed: Option<ChangedLines>,
    /// The commit the range starts from, for `Focus::base` (ADR-103 §5).
    base_commit: Option<String>,
    /// Per language, the symbols `--symbols` selected; empty without selectors.
    selected: HashMap<LangId, BTreeSet<crate::syntax::SymbolKey>>,
    files: Vec<String>,
    made: HashMap<(FactKind, LangId), std::result::Result<SharedFact, Error>>,
    /// Producers' findings, once per ID.
    findings: Vec<Finding>,
    seen: HashSet<String>,
    interrupted: bool,
    /// Every `TestResults` produced, by language.
    test_results: Vec<(LangId, TestResults)>,
    /// Every `MutationOutcomes` produced, by language.
    mutation_outcomes: Vec<(LangId, MutationOutcomes)>,
    /// The first red `TestResults`, which no check may read.
    refused: Option<Unevaluable>,
    /// Per language, the facts the run's tasks reach, which decide when a joint run saves a
    /// run (ADR-084); empty for a verb's production.
    reached: HashMap<LangId, BTreeSet<FactKind>>,
    /// The joint producers already run, by ID and language: each runs at most once.
    joints_run: HashSet<(String, LangId)>,
    /// How many processes or threads producers may run at once (`--jobs`).
    jobs: Option<NonZeroUsize>,
    /// How long each production took, for the store's history (ADR-086 §2).
    durations: Vec<TaskDuration>,
    oracle: Option<RunOracle>,
    cases: Option<u64>,
}

impl<'a> FactStore<'a> {
    /// The head side: the repository root and the files in scope.
    fn new(producers: &'a ProducerRegistry, job: &Job<'a>) -> Self {
        Self::over(
            producers,
            job,
            job.writer.root().to_path_buf(),
            job.files.clone(),
        )
    }

    /// The base side of a range: the base versions of the changed files (ADR-056).
    fn at(producers: &'a ProducerRegistry, job: &Job<'a>, base: &BaseTree) -> Self {
        Self::over(producers, job, base.root.clone(), base.files.clone())
    }

    fn over(
        producers: &'a ProducerRegistry,
        job: &Job<'a>,
        root: std::path::PathBuf,
        files: Vec<String>,
    ) -> Self {
        Self {
            producers,
            root: Ctx {
                root,
                exclude: job.config.file.scope.exclude.clone(),
                lang: job.config.file.lang.clone(),
            },
            writer: job.writer.confined(),
            cancel: job.cancel.clone(),
            config: job.config,
            seed: job.seed,
            depth: job.depth,
            changed: job.changed.clone(),
            base_commit: job.base.as_ref().map(|base| base.range.commit.clone()),
            selected: HashMap::new(),
            files,
            made: HashMap::new(),
            findings: Vec::new(),
            seen: HashSet::new(),
            interrupted: false,
            test_results: Vec::new(),
            mutation_outcomes: Vec::new(),
            refused: None,
            reached: HashMap::new(),
            joints_run: HashSet::new(),
            jobs: job.resources.jobs,
            durations: Vec::new(),
            oracle: job.oracle.clone(),
            cases: job.cases,
        }
    }

    /// Per language, every fact the tasks' checks reach, through their own languages and
    /// their family siblings'.
    fn reached_by(&self, tasks: &[Task<'_>]) -> HashMap<LangId, BTreeSet<FactKind>> {
        let mut reached: HashMap<LangId, BTreeSet<FactKind>> = HashMap::new();
        for task in tasks {
            let kinds = manifest_kinds(task.check.manifest());
            for lang in std::iter::once(&task.lang).chain(&task.siblings) {
                let found = self.deciding(lang).reached(&kinds);
                reached.entry(lang.id.clone()).or_default().extend(found);
            }
        }
        reached
    }

    /// What deciding how to make a fact for `lang` sees: the facts made so far among them.
    fn deciding<'s>(&'s self, lang: &'s Lang) -> Deciding<'s> {
        Deciding {
            producers: self.producers,
            lang,
            ctx: &self.root,
            depth: self.depth,
            upstream: self.upstream(self.made_kinds(lang).iter(), lang),
        }
    }

    /// The kinds made for `lang` so far.
    fn made_kinds(&self, lang: &Lang) -> Vec<FactKind> {
        self.made
            .iter()
            .filter(|((_, made), fact)| *made == lang.id && fact.is_ok())
            .map(|((kind, _), _)| *kind)
            .collect()
    }

    /// `kind` for `lang`, produced on first request by the first producer whose needs can be
    /// produced and which is ready, after producing those needs (ADR-041).
    fn ensure(
        &mut self,
        kind: FactKind,
        lang: &Lang,
        sink: &mut dyn EventSink,
    ) -> Result<SharedFact> {
        let key = (kind, lang.id.clone());
        if let Some(made) = self.made.get(&key) {
            return made.clone();
        }
        if let Some(made) = self.make_jointly(kind, lang, sink) {
            return made;
        }
        let made = self.produce(kind, lang, sink, &mut vec![kind]);
        self.keep(kind, lang, made)
    }

    /// Keeps `made` as `kind` for `lang`: a `TestResults` passes the gate, a
    /// `MutationOutcomes` is kept for the store's index, and a cancellation marks the run cut
    /// short.
    fn keep(
        &mut self,
        kind: FactKind,
        lang: &Lang,
        mut made: Result<SharedFact>,
    ) -> Result<SharedFact> {
        if matches!(made, Err(Error::Cancelled)) {
            self.interrupted = true;
        }
        let results = made
            .as_ref()
            .ok()
            .and_then(|fact| fact.downcast_ref::<TestResults>())
            .cloned();
        if let Some(results) = results {
            made = self.gate(made, results, lang);
        }
        if let Some(outcomes) = made
            .as_ref()
            .ok()
            .and_then(|fact| fact.downcast_ref::<MutationOutcomes>())
        {
            self.mutation_outcomes
                .push((lang.id.clone(), outcomes.clone()));
        }
        self.made.insert((kind, lang.id.clone()), made.clone());
        made
    }

    /// `kind` for `lang` made by a joint producer (ADR-084), with every other fact its run made;
    /// `None` when no joint applies, or the one that ran did not make `kind`, so `kind` is left
    /// to its own producer.
    fn make_jointly(
        &mut self,
        kind: FactKind,
        lang: &Lang,
        sink: &mut dyn EventSink,
    ) -> Option<Result<SharedFact>> {
        let reached = self.reached.get(&lang.id).cloned().unwrap_or_default();
        let made = |kind: FactKind| self.made.contains_key(&(kind, lang.id.clone()));
        let id = self
            .deciding(lang)
            .joint(kind, &reached, &made)
            .map(|(joint, _)| joint.id())?;
        let producers = self.producers;
        let joint = producers.joints_for(lang).find(|joint| joint.id() == id)?;
        if !self.joints_run.insert((id.to_string(), lang.id.clone())) {
            return None;
        }
        let mut visiting = vec![kind];
        let needs_met = joint
            .needs()
            .iter()
            .all(|need| self.ensure_need(*need, lang, sink, &mut visiting));
        if !needs_met {
            return None;
        }
        if let Err(error) = self.cancel.checkpoint() {
            return Some(self.keep(kind, lang, Err(error)));
        }
        let ctx = self.produce_ctx(joint.needs().iter(), lang);
        let estimate = joint.estimate(&self.root, &ctx.upstream);
        let scope = Scope::of_files(self.files.clone());
        let produced = self.timed(&joint.id(), lang, &estimate, sink, &mut |events| {
            joint.produce(&ctx, &scope, events)
        });
        match produced {
            Err(Error::Cancelled) => Some(self.keep(kind, lang, Err(Error::Cancelled))),
            Err(_) => None,
            Ok(facts) => {
                for fact in facts {
                    let made = fact.kind();
                    if !self.made.contains_key(&(made, lang.id.clone())) {
                        let _ = self.keep(made, lang, Ok(fact.share()));
                    }
                }
                self.made.get(&(kind, lang.id.clone())).cloned()
            }
        }
    }

    /// Makes `need` for `lang` for a producer, unless it is one of the kinds being made above
    /// (`visiting`), which would be a cycle; whether it was made.
    fn ensure_need(
        &mut self,
        need: FactKind,
        lang: &Lang,
        sink: &mut dyn EventSink,
        visiting: &mut Vec<FactKind>,
    ) -> bool {
        if visiting.contains(&need) {
            return false;
        }
        visiting.push(need);
        let met = self.ensure(need, lang, sink).is_ok();
        visiting.pop();
        met
    }

    /// What a producer of `lang` sees: the run's context, with the facts of `inputs` (its needs
    /// and wants) already made.
    fn produce_ctx<'k>(
        &self,
        inputs: impl Iterator<Item = &'k FactKind>,
        lang: &Lang,
    ) -> ProduceCtx {
        ProduceCtx {
            ctx: self.root.clone(),
            config: self.config.lang_config(&lang.id),
            writer: self.writer.clone(),
            cancel: self.cancel.clone(),
            upstream: self.upstream(inputs, lang),
            seed: self.seed,
            runners: self.producers.runners(),
            focus: Focus {
                depth: self.depth,
                symbols: self.selected.get(&lang.id).cloned(),
                changed: self.changed.clone(),
                base: self.base_commit.clone(),
                jobs: self.jobs,
                cases: self.cases,
            },
            oracle: self.oracle.clone(),
        }
    }

    /// Keeps a produced `TestResults` for the store and withholds a red one, recording it as
    /// the refusal, so nothing reads a red baseline (ADR-076 §5).
    fn gate(
        &mut self,
        made: Result<SharedFact>,
        results: TestResults,
        lang: &Lang,
    ) -> Result<SharedFact> {
        self.test_results.push((lang.id.clone(), results.clone()));
        if results.is_green() {
            return made;
        }
        self.refused.get_or_insert(Unevaluable {
            lang: lang.id.clone(),
            results,
        });
        Err(Error::AbsentFact {
            kind: FactKind::TestResults,
        })
    }

    fn produce(
        &mut self,
        kind: FactKind,
        lang: &Lang,
        sink: &mut dyn EventSink,
        visiting: &mut Vec<FactKind>,
    ) -> Result<SharedFact> {
        let producers = self
            .producers
            .candidates(kind, lang, &self.root)
            .map_err(|_| Error::AbsentFact { kind })?;
        for producer in producers {
            if producer.availability(&self.root) != Availability::Ready {
                continue;
            }
            let needs_met = producer
                .needs()
                .iter()
                .all(|need| self.ensure_need(*need, lang, sink, visiting));
            if !needs_met {
                continue;
            }
            self.ensure_wants(producer.wants(), lang, sink, visiting);
            self.cancel.checkpoint()?;
            let ctx = self.produce_ctx(producer.needs().iter().chain(producer.wants()), lang);
            let estimate = producer.estimate(&self.root, &ctx.upstream);
            let scope = Scope::of_files(self.files.clone());
            let produced = self.timed(&producer.id(), lang, &estimate, sink, &mut |events| {
                producer.produce(&ctx, &scope, events)
            });
            return produced.map(crate::fact::Produced::share);
        }
        Err(Error::AbsentFact { kind })
    }

    /// Makes each of `wants` for `lang` that can be made by a producer within the run's depth
    /// (ADR-080), as the producer estimates it here (a cache to refresh can make a `deep`
    /// producer `standard`, ADR-083); one that cannot is left out, so an optional fact never
    /// costs more than the run asked for.
    fn ensure_wants(
        &mut self,
        wants: &[FactKind],
        lang: &Lang,
        sink: &mut dyn EventSink,
        visiting: &mut Vec<FactKind>,
    ) {
        for want in wants {
            let upstream = self.upstream(self.made_kinds(lang).iter(), lang);
            let affordable = self.made.contains_key(&(*want, lang.id.clone()))
                || self
                    .producers
                    .producers_for(*want, lang)
                    .any(|producer| producer.estimate(&self.root, &upstream).cost <= self.depth);
            if affordable && !visiting.contains(want) {
                visiting.push(*want);
                let _ = self.ensure(*want, lang, sink);
                visiting.pop();
            }
        }
    }

    /// The facts of `needs` already made for `lang`, for a producer that needs or wants them.
    fn upstream<'k>(&self, needs: impl Iterator<Item = &'k FactKind>, lang: &Lang) -> Upstream {
        let mut upstream = Upstream::default();
        for fact in needs
            .filter_map(|need| self.made.get(&(*need, lang.id.clone())))
            .filter_map(|made| made.as_ref().ok())
        {
            upstream.insert(fact.clone());
        }
        upstream
    }

    /// Whether `lang`'s baseline was green and ran no test, so nothing that depends on test
    /// execution can be measured there.
    fn ran_no_test(&self, lang: &LangId) -> bool {
        self.test_results
            .iter()
            .any(|(ran, results)| ran == lang && results.is_green() && results.ran_none())
    }

    /// Passes a producer's event on, keeping each finding once by ID.
    fn forward(&mut self, event: Event, sink: &mut dyn EventSink) {
        if let Event::Finding(finding) = &event {
            if !self.seen.insert(finding.id.to_string()) {
                return;
            }
            self.findings.push((**finding).clone());
        }
        sink.emit(event);
    }
}

/// One check across its languages: per-language results, findings, and progress.
struct CheckRun<'a> {
    check: &'a dyn Check,
    targets: u64,
    done: u64,
    results: Vec<CheckResult>,
    findings: Vec<Finding>,
    measurements: Vec<Measured>,
    started: Instant,
}

/// What one check produced across its languages.
struct Finished {
    /// The check's one entry (ADR-064).
    result: CheckResult,
    findings: Vec<Finding>,
    measurements: Vec<Measured>,
}

impl<'a> CheckRun<'a> {
    fn start(check: &'a dyn Check, tasks: &[&Task<'_>], sink: &mut dyn EventSink) -> Self {
        let targets = tasks.iter().map(|task| task.plan.targets).sum();
        sink.emit(Event::CheckStart {
            id: check.manifest().id.clone(),
            targets,
        });
        Self {
            check,
            targets,
            done: 0,
            results: Vec::new(),
            findings: Vec::new(),
            measurements: Vec::new(),
            started: Instant::now(),
        }
    }

    fn sink<'s>(&'s mut self, lang: &'s Lang, forward: &'s mut dyn EventSink) -> CheckSink<'s> {
        CheckSink {
            forward,
            check: self.check,
            lang,
            findings: &mut self.findings,
            measurements: &mut self.measurements,
        }
    }

    fn record(&mut self, task: &Task<'_>, mut result: CheckResult, sink: &mut dyn EventSink) {
        // Each language's notes say which language they are about once languages combine.
        for note in &mut result.notes {
            *note = format!("{note} ({})", task.lang.id);
        }
        self.done += task.plan.targets;
        sink.emit(Event::Progress {
            id: self.check.manifest().id.clone(),
            done: self.done,
            total: self.targets,
            eta_ms: None,
        });
        self.results.push(result);
    }

    /// The check's one entry (ADR-064), with its findings and measurements: its languages'
    /// results combined, `partial` with the first cut-short language's skip when the timeout or
    /// budget skipped a language, or one `skipped` entry when every language was skipped.
    /// `skips` are the languages the support matrix or a missing range ruled out.
    fn finish(self, mut skips: Vec<Skipped>, sink: &mut dyn EventSink) -> Finished {
        let manifest = self.check.manifest();
        let (skipped, ran): (Vec<CheckResult>, Vec<CheckResult>) = self
            .results
            .into_iter()
            .partition(|result| result.status == CheckStatus::Skipped);
        skips.extend(skipped.into_iter().filter_map(|result| result.skipped));
        let duration_ms = elapsed_ms(self.started);
        let result = if ran.is_empty() && !skips.is_empty() {
            CheckResult {
                duration_ms,
                ..skipped_entry(self.check, skips)
            }
        } else {
            let flagged = self
                .findings
                .iter()
                .any(|finding| finding.severity != Severity::Info);
            let cut = skips
                .into_iter()
                .filter(|skipped| {
                    matches!(skipped.reason, SkipReason::Timeout | SkipReason::Budget)
                })
                .min_by(|a, b| a.lang.cmp(&b.lang));
            let status = ran
                .iter()
                .map(|result| result.status)
                .chain(flagged.then_some(CheckStatus::Fail))
                .chain(cut.is_some().then_some(CheckStatus::Partial))
                .max_by_key(|status| rank(*status))
                .unwrap_or(CheckStatus::Pass);
            let scores: Vec<_> = ran.iter().filter_map(|r| r.score.clone()).collect();
            CheckResult {
                id: manifest.id.clone(),
                status,
                tier: manifest.tier,
                cost: manifest.cost,
                score: self.check.combine(&scores),
                metrics: combine_metrics(self.check, &ran),
                delta: None,
                findings: self.findings.len() as u64,
                duration_ms,
                skipped: cut,
                oracle: None,
                notes: combined_notes(&ran),
            }
        };
        sink.emit(Event::CheckEnd {
            id: manifest.id.clone(),
            status: result.status,
            duration_ms,
        });
        Finished {
            result,
            findings: self.findings,
            measurements: self.measurements,
        }
    }
}

/// Every language's notes, sorted, each once.
fn combined_notes(ran: &[CheckResult]) -> Vec<String> {
    let notes: BTreeSet<&String> = ran.iter().flat_map(|result| &result.notes).collect();
    notes.into_iter().cloned().collect()
}

/// Each `scope` metric any language reported, combined across languages by the check (ADR-060).
fn combine_metrics(check: &dyn Check, ran: &[CheckResult]) -> BTreeMap<String, f64> {
    let names: BTreeSet<&str> = ran
        .iter()
        .flat_map(|result| result.metrics.keys().map(String::as_str))
        .collect();
    names
        .into_iter()
        .filter_map(|name| {
            let reported: Vec<&CheckResult> = ran
                .iter()
                .filter(|result| result.metrics.contains_key(name))
                .collect();
            check
                .combine_metric(name, &reported)
                .map(|value| (name.to_owned(), value))
        })
        .collect()
}

/// What a check emits through: findings are kept for the envelope and streamed on;
/// measurements are kept with the check and language (ADR-062).
struct CheckSink<'a> {
    forward: &'a mut dyn EventSink,
    check: &'a dyn Check,
    lang: &'a Lang,
    findings: &'a mut Vec<Finding>,
    measurements: &'a mut Vec<Measured>,
}

impl EventSink for CheckSink<'_> {
    fn emit(&mut self, event: Event) {
        if let Event::Finding(finding) = &event {
            self.findings.push((**finding).clone());
        }
        self.forward.emit(event);
    }

    fn measure(&mut self, measurement: Measurement) {
        self.measurements.push(Measured {
            check: self.check.manifest().id.clone(),
            lang: self.lang.id.clone(),
            measurement,
        });
    }
}

/// How bad a status is, for combining per-language results.
fn rank(status: CheckStatus) -> u8 {
    match status {
        CheckStatus::Pass | CheckStatus::Skipped => 0,
        CheckStatus::Fail => 1,
        CheckStatus::Partial => 2,
        CheckStatus::Error => 3,
    }
}

fn status_of(error: &Error) -> CheckStatus {
    if *error == Error::Cancelled {
        CheckStatus::Partial
    } else {
        CheckStatus::Error
    }
}

fn same(a: &dyn Check, b: &dyn Check) -> bool {
    a.manifest().id == b.manifest().id
}

/// The one entry of a check every language skipped (ADR-064): the first language's skip in
/// language-ID order, naming no language when every language was skipped alike.
fn skipped_entry(check: &dyn Check, mut skips: Vec<Skipped>) -> CheckResult {
    skips.sort_by(|a, b| a.lang.cmp(&b.lang));
    let alike = |a: &Skipped, b: &Skipped| {
        (a.reason, &a.fact, &a.tool, &a.see) == (b.reason, &b.fact, &b.tool, &b.see)
    };
    let mut skipped = skips.first().cloned();
    if let Some(first) = skipped.as_mut()
        && skips.len() > 1
        && skips.iter().all(|other| alike(first, other))
    {
        first.lang = None;
        first.est_ms = first
            .est_ms
            .map(|_| skips.iter().filter_map(|skip| skip.est_ms).sum());
    }
    let manifest = check.manifest();
    CheckResult {
        id: manifest.id.clone(),
        status: CheckStatus::Skipped,
        tier: manifest.tier,
        cost: manifest.cost,
        score: None,
        metrics: BTreeMap::new(),
        delta: None,
        findings: 0,
        duration_ms: 0,
        skipped,
        oracle: None,
        notes: Vec::new(),
    }
}

/// The `skipped` object for a pair the support matrix rules out.
fn unsupported(support: &Support, lang: &LangId) -> Skipped {
    let (reason, fact, tool) = match support {
        Support::Unavailable(tool) => (SkipReason::Unavailable, None, Some(tool.clone())),
        Support::Unconfigured(_) => (SkipReason::Unconfigured, None, None),
        Support::Unsupported(fact) => (SkipReason::Unsupported, Some(*fact), None),
        Support::Supported => (SkipReason::Unsupported, None, None),
    };
    Skipped {
        reason,
        fact,
        lang: Some(lang.clone()),
        tool,
        see: Some("cupel describe matrix".to_owned()),
        est_ms: None,
    }
}

/// The `skipped` object for a check that compares a range's two sides on a scope without one.
fn needs_range(id: &crate::id::CheckId, lang: &LangId) -> Skipped {
    Skipped {
        reason: SkipReason::NeedsRange,
        fact: None,
        lang: Some(lang.clone()),
        tool: None,
        see: Some(format!("cupel check --changed --only {id}")),
        est_ms: None,
    }
}

/// The `skipped` object for a check that depends on test execution in a language whose
/// baseline ran no test: test results are unavailable there.
fn no_tests_ran(lang: &LangId) -> Skipped {
    Skipped {
        fact: Some(FactKind::TestResults),
        see: Some("cupel describe matrix".to_owned()),
        ..skip_reason(SkipReason::Unavailable, lang)
    }
}

fn skip_reason(reason: SkipReason, lang: &LangId) -> Skipped {
    Skipped {
        reason,
        fact: None,
        lang: Some(lang.clone()),
        tool: None,
        see: None,
        est_ms: None,
    }
}

/// Envelope order: by location, then check, kind, and ID; findings without a location last.
fn sort_findings(findings: &mut [Finding]) {
    findings.sort_by(|a, b| {
        let at = |finding: &Finding| {
            finding.location.as_ref().map(|location| {
                (
                    location.file.clone(),
                    location.line,
                    location.col.unwrap_or(0),
                )
            })
        };
        (at(a).is_none(), at(a), &a.check, &a.kind, a.id.as_str()).cmp(&(
            at(b).is_none(),
            at(b),
            &b.check,
            &b.kind,
            b.id.as_str(),
        ))
    });
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
