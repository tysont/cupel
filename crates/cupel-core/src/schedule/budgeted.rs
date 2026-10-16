// ABOUTME: The scheduler under a budget (ADR-086): costs each task with the producer runs it brings in, takes tasks by
// ABOUTME: the knapsack, skips the rest with `est_ms`, cuts running work at the deadline, and times what ran.
use std::collections::{BTreeSet, HashMap};
use std::time::Instant;

use super::{Execution, FactStore, Job, Prepared, Skip, Task, elapsed_ms, manifest_kinds};
use crate::budget::{Candidate, History, TaskDuration, TaskShape, choose, tie, value};
use crate::cancel::Cancellation;
use crate::error::Result;
use crate::event::{Event, EventSink};
use crate::fact::FactKind;
use crate::grade::CostClass;
use crate::id::{LangId, ProducerId};
use crate::joint::{CACHED, Deciding};
use crate::lang::Lang;
use crate::producer::Estimate;
use crate::result::{SkipReason, Skipped};

/// One producer run for one language: the producer (or joint producer) and the language.
type ProducerTask = (ProducerId, LangId);

/// Orders `prepared`'s tasks as the run takes them and gives each its cost with the producer
/// runs it brings in; under a budget, takes them by the knapsack and moves the rest to the skips
/// with reason `budget` (ADR-086 §5). Leaves the fact store reaching only what the taken tasks
/// reach (ADR-084).
pub(super) fn schedule(job: &Job<'_>, store: &mut FactStore<'_>, prepared: &mut Prepared<'_>) {
    store.reached = store.reached_by(&prepared.tasks);
    let history = &job.resources.history;
    let mut costing = Costing::new(store, history);
    let candidates: Vec<Candidate> = prepared
        .tasks
        .iter()
        .map(|task| Candidate {
            value: task_value(job, task),
            cost_ms: own_cost(task, history),
            closure: costing.closure(task),
            tie: tie(job.seed, &task.check.manifest().id, &task.lang.id),
        })
        .collect();
    let left = job
        .resources
        .budget
        .map(|budget| budget.left_ms(Instant::now()));
    let choice = choose(&candidates, &costing.costs, left);
    let mut tasks: Vec<Option<Task<'_>>> = prepared.tasks.drain(..).map(Some).collect();
    for (index, cost_ms) in choice.taken {
        if let Some(mut task) = tasks[index].take() {
            task.cost_ms = cost_ms;
            prepared.tasks.push(task);
        }
    }
    for (index, est_ms) in choice.skipped {
        if let Some(task) = tasks[index].take() {
            prepared.skips.push(Skip {
                check: task.check,
                skipped: skip(&task.lang.id, est_ms),
            });
        }
    }
    store.reached = store.reached_by(&prepared.tasks);
}

/// The run's cancellation, cancelled also at the budget's deadline when there is a budget: what
/// every check and production runs under, so work running at the deadline is cut (ADR-086 §6).
pub(super) fn cancellation(job: &Job<'_>) -> Cancellation {
    match job.resources.budget {
        Some(budget) => job
            .cancel
            .child(budget.deadline.saturating_duration_since(Instant::now())),
        None => job.cancel.clone(),
    }
}

/// The `skipped` object of a task the budget left out, with what it was estimated to cost.
pub(super) fn skip(lang: &LangId, est_ms: u64) -> Skipped {
    Skipped {
        reason: SkipReason::Budget,
        fact: None,
        lang: Some(lang.clone()),
        tool: None,
        see: None,
        est_ms: Some(est_ms),
    }
}

/// Whether a producer that estimates `estimate` would read its cache rather than run.
fn is_cached(estimate: &Estimate) -> bool {
    estimate.note.as_deref() == Some(CACHED)
}

/// What `task` is worth: its tier, its check's novelty, and whether a range changed what it
/// looks at (ADR-086 §4).
fn task_value(job: &Job<'_>, task: &Task<'_>) -> f64 {
    let manifest = task.check.manifest();
    let changed = job.changed.is_some()
        && task
            .scope
            .symbols
            .as_ref()
            .is_some_and(|symbols| !symbols.is_empty());
    value(
        manifest.tier,
        job.resources.history.novelty(&manifest.id),
        changed,
    )
}

/// What `task`'s check costs itself: its shape's recorded p50, else its plan's estimate.
fn own_cost(task: &Task<'_>, history: &History) -> u64 {
    let shape = TaskShape::check(&task.check.manifest().id, &task.lang.id, task.plan.targets);
    history.cost_or(&shape, task.plan.est_ms)
}

/// The producer runs tasks bring in, each costed once, by index.
struct Costing<'s, 'a> {
    store: &'s FactStore<'a>,
    history: &'s History,
    /// Each producer task's index into `costs`.
    index: HashMap<ProducerTask, usize>,
    costs: Vec<u64>,
    /// What makes each (kind, language), once looked up.
    made_by: HashMap<(FactKind, LangId), Option<(ProducerTask, u64)>>,
}

impl<'s, 'a> Costing<'s, 'a> {
    fn new(store: &'s FactStore<'a>, history: &'s History) -> Self {
        Self {
            store,
            history,
            index: HashMap::new(),
            costs: Vec::new(),
            made_by: HashMap::new(),
        }
    }

    /// The producer tasks of `task`'s closure: a run for every fact its check reaches, in its
    /// language and its family siblings', that is not made yet.
    fn closure(&mut self, task: &Task<'_>) -> Vec<usize> {
        let kinds = manifest_kinds(task.check.manifest());
        let mut closure = BTreeSet::new();
        for lang in std::iter::once(&task.lang).chain(&task.siblings) {
            let reached = self.store.deciding(lang).reached(&kinds);
            for kind in reached {
                if self.store.made.contains_key(&(kind, lang.id.clone())) {
                    continue;
                }
                if let Some((producer, cost)) = self.made_by(kind, lang) {
                    let next = self.costs.len();
                    let index = *self.index.entry(producer).or_insert(next);
                    if index == next {
                        self.costs.push(cost);
                    }
                    closure.insert(index);
                }
            }
        }
        closure.into_iter().collect()
    }

    fn made_by(&mut self, kind: FactKind, lang: &Lang) -> Option<(ProducerTask, u64)> {
        let key = (kind, lang.id.clone());
        if let Some(found) = self.made_by.get(&key) {
            return found.clone();
        }
        let found = production(self.store, kind, lang, self.history);
        self.made_by.insert(key, found.clone());
        found
    }
}

/// The producer run that would make `kind` for `lang` now, with its cost: the joint run that
/// would make it (ADR-084), else the producer the store would choose; `None` when nothing can.
fn production(
    store: &FactStore<'_>,
    kind: FactKind,
    lang: &Lang,
    history: &History,
) -> Option<(ProducerTask, u64)> {
    let deciding: Deciding<'_> = store.deciding(lang);
    let reached = store.reached.get(&lang.id).cloned().unwrap_or_default();
    let made = |kind: FactKind| store.made.contains_key(&(kind, lang.id.clone()));
    let (id, estimate) = if let Some((joint, _)) = deciding.joint(kind, &reached, &made) {
        (joint.id(), joint.estimate(deciding.ctx, &deciding.upstream))
    } else {
        let producer = deciding.chosen(kind)?;
        (
            producer.id(),
            producer.estimate(deciding.ctx, &deciding.upstream),
        )
    };
    let cached = is_cached(&estimate);
    let default = if cached {
        CostClass::Quick
    } else {
        estimate.cost
    }
    .est_ms_per_production();
    let shape = TaskShape::producer(&id, &lang.id, cached);
    let cost = history.cost_or(&shape, default);
    Some(((id, lang.id.clone()), cost))
}

impl FactStore<'_> {
    /// Runs `produce`, `producer`'s production for `lang`, forwarding the events it emitted and
    /// recording how long it took when it succeeded, under its shape (ADR-086 §2): `estimate`
    /// says whether it would read its cache.
    pub(super) fn timed<T>(
        &mut self,
        producer: &ProducerId,
        lang: &Lang,
        estimate: &Estimate,
        sink: &mut dyn EventSink,
        produce: &mut dyn FnMut(&mut Vec<Event>) -> Result<T>,
    ) -> Result<T> {
        let mut events = Vec::new();
        let started = Instant::now();
        let result = produce(&mut events);
        if result.is_ok() {
            self.durations.push(TaskDuration {
                shape: TaskShape::producer(producer, &lang.id, is_cached(estimate)),
                duration_ms: elapsed_ms(started),
            });
        }
        for event in events {
            self.forward(event, sink);
        }
        result
    }
}

impl Execution<'_, '_> {
    /// The estimate of `task` when it no longer fits what is left of the budget: its own cost
    /// plus the producer runs it would still bring in (ADR-086 §6); `None` when it fits or there
    /// is no budget.
    pub(super) fn over_budget(&self, task: &Task<'_>) -> Option<u64> {
        let budget = self.job.resources.budget?;
        let history = &self.job.resources.history;
        let mut costing = Costing::new(&self.store, history);
        let closure = costing.closure(task);
        let est_ms = closure.iter().fold(own_cost(task, history), |sum, index| {
            sum.saturating_add(costing.costs[*index])
        });
        let left = budget.left_ms(Instant::now());
        (left == 0 || est_ms > left).then_some(est_ms)
    }
}
