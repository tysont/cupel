// ABOUTME: Scheduler v1 (design §8.9, ADR-041): resolve the symbols in scope, split selected checks into
// ABOUTME: supported (check, language) tasks and skips, produce each needed fact once, run, and combine.
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::time::Instant;

use crate::base::BaseTree;
use crate::cancel::Cancellation;
use crate::check::{Check, CheckCtx, CheckRegistry, Plan};
use crate::config::Config;
use crate::dry_run::{PlannedCheck, PlannedSkip, RunPlan};
use crate::error::{ArgumentError, Error, Result};
use crate::event::{Event, EventSink};
use crate::fact::FactKind;
use crate::facts::Facts;
use crate::finding::{Finding, Severity};
use crate::id::{LangId, RunId};
use crate::lang::Lang;
use crate::matrix::{Support, SupportMatrix};
use crate::producer::{Availability, Ctx, ProduceCtx, ProducerRegistry, SharedFact};
use crate::rank::{Measured, Measurement};
use crate::result::{CheckResult, CheckStatus, SkipReason, Skipped};
use crate::scope::{Scope, SymbolSelector, select_symbols};
use crate::syntax::{NormTree, Symbols};
use crate::writer::Writer;

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
}

/// One (check, language) the support matrix allows, with its plan.
struct Task<'a> {
    check: &'a dyn Check,
    lang: Lang,
    /// The family's other supported languages, for a family-wide check (ADR-051).
    siblings: Vec<Lang>,
    scope: Scope,
    plan: Plan,
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
        let mut base = job
            .base
            .as_ref()
            .map(|base| FactStore::at(self.producers, job, base));
        let prepared = self.prepare(job, &mut store, sink)?;
        let est_ms = prepared.tasks.iter().map(|task| task.plan.est_ms).sum();
        sink.emit(Event::Plan {
            run: job.run,
            checks: u32::try_from(job.selected.len()).unwrap_or(u32::MAX),
            est_ms,
        });
        for skip in &prepared.skips {
            sink.emit(Event::Skip {
                id: skip.check.manifest().id.clone(),
                reason: skip.skipped.reason,
            });
        }
        let mut checks = Vec::new();
        let mut findings = Vec::new();
        let mut measurements = Vec::new();
        let mut interrupted = store.interrupted;
        for check in &job.selected {
            let tasks: Vec<&Task<'_>> = prepared
                .tasks
                .iter()
                .filter(|task| same(task.check, *check))
                .collect();
            let skips: Vec<&Skip<'_>> = prepared
                .skips
                .iter()
                .filter(|skip| same(skip.check, *check))
                .collect();
            let skips: Vec<Skipped> = skips.into_iter().map(|skip| skip.skipped.clone()).collect();
            // A check runs when some language supports it, or when no language is in scope.
            if tasks.is_empty() && !skips.is_empty() {
                checks.push(skipped_entry(*check, skips));
            } else {
                let mut run = CheckRun::start(*check, &tasks, sink);
                for task in tasks {
                    let result = Self::run_task(
                        job,
                        &mut store,
                        base.as_mut(),
                        task,
                        &mut run.sink(&task.lang, sink),
                    );
                    // A check may skip itself for its own reasons (ADR-048); only a cut-short
                    // check or a timeout skip makes the run partial.
                    interrupted |= result.status == CheckStatus::Partial
                        || result
                            .skipped
                            .as_ref()
                            .is_some_and(|skipped| skipped.reason == SkipReason::Timeout);
                    run.record(task, result, sink);
                }
                let finished = run.finish(skips, sink);
                checks.push(finished.result);
                findings.extend(finished.findings);
                measurements.extend(finished.measurements);
            }
        }
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
        })
    }

    /// Plans `job` without running a check: what would run, with targets and estimates, and
    /// what would be skipped. Resolving the scope parses files but writes nothing.
    pub fn plan(&self, job: &Job<'_>, sink: &mut dyn EventSink) -> Result<Outcome> {
        let mut store = FactStore::new(self.producers, job);
        let prepared = self.prepare(job, &mut store, sink)?;
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
                Some(PlannedCheck {
                    id: manifest.id.clone(),
                    targets: tasks.iter().map(|task| task.plan.targets).sum(),
                    est_ms: tasks.iter().map(|task| task.plan.est_ms).sum(),
                    needs: manifest
                        .needs
                        .iter()
                        .chain(&manifest.wants)
                        .map(ToString::to_string)
                        .collect(),
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
        })
    }

    /// Resolves the symbols in scope, then splits the selected checks into tasks and skips.
    fn prepare<'a>(
        &self,
        job: &'a Job<'a>,
        store: &mut FactStore<'_>,
        sink: &mut dyn EventSink,
    ) -> Result<Prepared<'a>> {
        let scopes = resolve_symbols(job, store, sink)?;
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
            },
        );
        let mut tasks = Vec::new();
        let mut skips = Vec::new();
        for check in &job.selected {
            let manifest = check.manifest();
            let mut supported = Vec::new();
            for lang in &job.langs {
                let support = matrix
                    .get(&manifest.id, &lang.id)
                    .cloned()
                    .unwrap_or(Support::Supported);
                if support == Support::Supported {
                    supported.push(lang.clone());
                } else {
                    skips.push(Skip {
                        check: *check,
                        skipped: unsupported(&support, &lang.id),
                    });
                }
            }
            for mut langs in task_groups(*check, supported) {
                let lang = langs.remove(0);
                let scope = langs.iter().fold(
                    scopes.get(&lang.id).cloned().unwrap_or_default(),
                    |scope, sibling| union(scope, scopes.get(&sibling.id)),
                );
                if !check.base_needs().is_empty() && job.base.is_none() {
                    skips.push(Skip {
                        check: *check,
                        skipped: needs_range(&manifest.id, &lang.id),
                    });
                    continue;
                }
                let ctx = check_ctx(job, *check, &lang.id, scope.clone());
                let plan = check.plan(&ctx, &scope)?;
                tasks.push(Task {
                    check: *check,
                    lang,
                    siblings: langs,
                    scope,
                    plan,
                });
            }
        }
        Ok(Prepared {
            tasks,
            skips,
            symbols,
        })
    }

    /// Produces what `task` reads and runs it; never fails, since one check's failure must not
    /// stop the others (ADR-041).
    fn run_task(
        job: &Job<'_>,
        store: &mut FactStore<'_>,
        base: Option<&mut FactStore<'_>>,
        task: &Task<'_>,
        sink: &mut CheckSink<'_>,
    ) -> CheckResult {
        let manifest = task.check.manifest();
        let started = Instant::now();
        let result = |status, skipped| CheckResult {
            id: manifest.id.clone(),
            status,
            tier: manifest.tier,
            cost: manifest.cost,
            score: None,
            metrics: BTreeMap::new(),
            delta: None,
            findings: 0,
            duration_ms: elapsed_ms(started),
            skipped,
            oracle: None,
        };
        if job.cancel.is_cancelled() {
            return result(
                CheckStatus::Skipped,
                Some(skip_reason(SkipReason::Timeout, &task.lang.id)),
            );
        }
        let mut facts = Facts::for_manifest(manifest);
        for kind in &manifest.needs {
            match store.ensure(*kind, &task.lang, &mut *sink.forward) {
                Ok(fact) => {
                    if let Err(error) = facts.insert_shared(&fact) {
                        return result(status_of(&error), None);
                    }
                }
                Err(error) => return result(status_of(&error), None),
            }
        }
        for kind in &manifest.wants {
            if let Ok(fact) = store.ensure(*kind, &task.lang, &mut *sink.forward) {
                let _ = facts.insert_shared(&fact);
            }
        }
        for sibling in &task.siblings {
            for kind in &manifest.needs {
                match store.ensure(*kind, sibling, &mut *sink.forward) {
                    Ok(fact) => {
                        if let Err(error) = facts.insert_shared_sibling(&fact) {
                            return result(status_of(&error), None);
                        }
                    }
                    Err(error) => return result(status_of(&error), None),
                }
            }
            for kind in &manifest.wants {
                if let Ok(fact) = store.ensure(*kind, sibling, &mut *sink.forward) {
                    let _ = facts.insert_shared_sibling(&fact);
                }
            }
        }
        if let Some(tree) = &job.base {
            facts.set_range(tree.range.clone());
        }
        if let Some(base) = base {
            for kind in task.check.base_needs() {
                // Base-side producer findings describe code the change replaced: dropped.
                match base.ensure(*kind, &task.lang, &mut Vec::new()) {
                    Ok(fact) => {
                        if let Err(error) = facts.insert_base_shared(&fact) {
                            return result(status_of(&error), None);
                        }
                    }
                    Err(error) => return result(status_of(&error), None),
                }
            }
        }
        let ctx = check_ctx(job, task.check, &task.lang.id, task.scope.clone());
        match task.check.run(&ctx, &facts, &task.plan, sink) {
            Ok(mut ran) => {
                ran.duration_ms = elapsed_ms(started);
                ran
            }
            Err(error) => result(status_of(&error), None),
        }
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

/// The symbols in scope per language: every symbol of its files, or those the selectors match.
fn resolve_symbols(
    job: &Job<'_>,
    store: &mut FactStore<'_>,
    sink: &mut dyn EventSink,
) -> Result<HashMap<LangId, Scope>> {
    let needs_hashes = job.selectors.iter().any(SymbolSelector::needs_hashes);
    let mut matched = vec![false; job.selectors.len()];
    let mut scopes = HashMap::new();
    for lang in &job.langs {
        let Ok(symbols) = store.ensure(FactKind::Symbols, lang, sink) else {
            scopes.insert(lang.id.clone(), scope_of(job, BTreeSet::new()));
            continue;
        };
        let Some(symbols) = symbols.downcast_ref::<Symbols>() else {
            scopes.insert(lang.id.clone(), scope_of(job, BTreeSet::new()));
            continue;
        };
        let keys = if job.selectors.is_empty() {
            symbols
                .symbols
                .iter()
                .map(crate::syntax::SymbolDef::key)
                .collect()
        } else {
            let norm = if needs_hashes {
                store.ensure(FactKind::NormTree, lang, sink).ok()
            } else {
                None
            };
            let norm = norm
                .as_ref()
                .and_then(|fact| fact.downcast_ref::<NormTree>());
            let (keys, hits) = select_symbols(&job.selectors, symbols, norm);
            for (seen, hit) in matched.iter_mut().zip(hits) {
                *seen |= hit;
            }
            keys
        };
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
    files: Vec<String>,
    made: HashMap<(FactKind, LangId), std::result::Result<SharedFact, Error>>,
    /// Producers' findings, once per ID.
    findings: Vec<Finding>,
    seen: HashSet<String>,
    interrupted: bool,
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
            },
            writer: job.writer.confined(),
            cancel: job.cancel.clone(),
            config: job.config,
            files,
            made: HashMap::new(),
            findings: Vec::new(),
            seen: HashSet::new(),
            interrupted: false,
        }
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
        let made = self.produce(kind, lang, sink, &mut vec![kind]);
        if matches!(made, Err(Error::Cancelled)) {
            self.interrupted = true;
        }
        self.made.insert(key, made.clone());
        made
    }

    fn produce(
        &mut self,
        kind: FactKind,
        lang: &Lang,
        sink: &mut dyn EventSink,
        visiting: &mut Vec<FactKind>,
    ) -> Result<SharedFact> {
        let producers: Vec<_> = self.producers.producers_for(kind, lang).collect();
        for producer in producers {
            if producer.availability(&self.root) != Availability::Ready {
                continue;
            }
            let needs_met = producer.needs().iter().all(|need| {
                !visiting.contains(need) && {
                    visiting.push(*need);
                    let met = self.ensure(*need, lang, sink).is_ok();
                    visiting.pop();
                    met
                }
            });
            if !needs_met {
                continue;
            }
            self.cancel.checkpoint()?;
            let ctx = ProduceCtx {
                ctx: self.root.clone(),
                config: self.config.lang_config(&lang.id),
                writer: self.writer.clone(),
                cancel: self.cancel.clone(),
            };
            let mut events = Vec::new();
            let produced =
                producer.produce(&ctx, &Scope::of_files(self.files.clone()), &mut events);
            for event in events {
                self.forward(event, sink);
            }
            return produced.map(crate::producer::Produced::share);
        }
        Err(Error::AbsentFact { kind })
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

    fn record(&mut self, task: &Task<'_>, result: CheckResult, sink: &mut dyn EventSink) {
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
            let scores: Vec<_> = ran
                .iter()
                .filter_map(|result| result.score.clone())
                .collect();
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
    }
}

fn skip_reason(reason: SkipReason, lang: &LangId) -> Skipped {
    Skipped {
        reason,
        fact: None,
        lang: Some(lang.clone()),
        tool: None,
        see: None,
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
