// ABOUTME: The budget scheduler's model (design §9.6, ADR-086): task shapes and recorded durations, cost as the p50
// ABOUTME: of recent runs else a class default, value per check task, and the greedy knapsack with dependency closure.
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use crate::grade::Tier;
use crate::id::{CheckId, LangId, ProducerId};

/// How many of a shape's most recent durations its cost is the p50 of.
pub const RECENT: usize = 10;

/// How many recorded durations a shape needs before they replace the default: one or two runs
/// are mostly a cold build or a cold cache.
pub const MIN_RUNS: usize = 3;

/// What a recorded duration timed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TaskKind {
    /// One check's `run` for one language, after its facts were made.
    Check,
    /// One producer's (or joint producer's) production for one language, after its inputs.
    Producer,
}

/// Each kind with the name the store keeps it under.
const TASK_KINDS: [(TaskKind, &str); 2] =
    [(TaskKind::Check, "check"), (TaskKind::Producer, "producer")];

impl TaskKind {
    /// The name the store keeps it under.
    pub fn as_str(self) -> &'static str {
        TASK_KINDS
            .iter()
            .find(|(kind, _)| *kind == self)
            .map_or("", |(_, name)| name)
    }

    /// The kind named `name`, as the store keeps it.
    pub fn named(name: &str) -> Option<Self> {
        TASK_KINDS
            .iter()
            .find(|(_, named)| *named == name)
            .map(|(kind, _)| *kind)
    }
}

/// Tasks whose durations predict each other (ADR-086 §1): a check for one language at one order
/// of magnitude of targets, or a producer for one language that runs or reads its cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskShape {
    pub kind: TaskKind,
    /// The check or producer ID.
    pub task: String,
    pub lang: LangId,
    /// A check's size class (`0`, then `1 + ⌊log₁₀ targets⌋`), or a producer's `run` | `cached`.
    pub class: String,
}

impl TaskShape {
    /// `check` for `lang` over `targets` targets.
    pub fn check(check: &CheckId, lang: &LangId, targets: u64) -> Self {
        let class = if targets == 0 {
            0
        } else {
            targets.ilog10() + 1
        };
        Self {
            kind: TaskKind::Check,
            task: check.to_string(),
            lang: lang.clone(),
            class: class.to_string(),
        }
    }

    /// `producer` for `lang`, reading its cache (`cached`) or running.
    pub fn producer(producer: &ProducerId, lang: &LangId, cached: bool) -> Self {
        Self {
            kind: TaskKind::Producer,
            task: producer.to_string(),
            lang: lang.clone(),
            class: if cached { "cached" } else { "run" }.to_owned(),
        }
    }
}

/// One measured duration of a task of `shape`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDuration {
    pub shape: TaskShape,
    pub duration_ms: u64,
}

/// Whether a check's findings would be new (ADR-086 §4). The discriminant is the power of two
/// its value is multiplied by, plus one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Novelty {
    /// The store holds no result of the check that ran: ×2.
    #[default]
    Unscored = 2,
    /// Its latest result was made at another state of the tree: ×1.
    Scored = 1,
    /// Its latest result was made at this commit with a clean tree, which is still clean: ×½.
    Repeated = 0,
}

impl Novelty {
    fn factor(self) -> f64 {
        2_f64.powi(self as i32 - 1)
    }
}

/// What the store recorded of earlier runs, for costing and valuing tasks: each shape's recent
/// durations, newest first, and each check's novelty.
#[derive(Debug, Clone, Default)]
pub struct History {
    recent: HashMap<TaskShape, Vec<u64>>,
    novelty: HashMap<CheckId, Novelty>,
}

impl History {
    /// Records `durations` of `shape`, newest first; only the first [`RECENT`] count.
    pub fn insert(&mut self, shape: TaskShape, mut durations: Vec<u64>) {
        durations.truncate(RECENT);
        self.recent.insert(shape, durations);
    }

    pub fn set_novelty(&mut self, check: CheckId, novelty: Novelty) {
        self.novelty.insert(check, novelty);
    }

    /// `check`'s novelty; unscored when the store said nothing about it.
    pub fn novelty(&self, check: &CheckId) -> Novelty {
        self.novelty.get(check).copied().unwrap_or_default()
    }

    /// The p50 of `shape`'s recent durations, when at least [`MIN_RUNS`] are recorded.
    pub fn cost_ms(&self, shape: &TaskShape) -> Option<u64> {
        self.recent
            .get(shape)
            .filter(|durations| durations.len() >= MIN_RUNS)
            .and_then(|durations| p50(durations))
    }

    /// `shape`'s recorded cost, else `default`.
    pub fn cost_or(&self, shape: &TaskShape, default: u64) -> u64 {
        self.cost_ms(shape).unwrap_or(default)
    }
}

/// The lower median of `durations`.
pub fn p50(durations: &[u64]) -> Option<u64> {
    let mut sorted = durations.to_vec();
    sorted.sort_unstable();
    sorted.get(sorted.len().checked_sub(1)? / 2).copied()
}

/// What a check task is worth: `tier_weight × novelty × (changed ? 2 : 1)`, with
/// `tier_weight = tier + 1` (ADR-086 §4).
pub fn value(tier: Tier, novelty: Novelty, changed: bool) -> f64 {
    let tier_weight = f64::from(tier.value()) + 1.0;
    tier_weight * novelty.factor() * if changed { 2.0 } else { 1.0 }
}

/// The tie-breaking key of `check`'s task for `lang` in a run seeded `seed`: equal ratios are
/// taken in key order, so `--seed` repeats a run's order.
pub fn tie(seed: u64, check: &CheckId, lang: &LangId) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&seed.to_le_bytes());
    hasher.update(check.as_str().as_bytes());
    hasher.update(&[0]);
    hasher.update(lang.as_str().as_bytes());
    let digest = hasher.finalize();
    let mut key = [0; 8];
    key.copy_from_slice(&digest.as_bytes()[..8]);
    u64::from_le_bytes(key)
}

/// A run's wall-clock budget: its length, and when it ends (the invocation's start plus it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub limit: Duration,
    pub deadline: Instant,
}

impl Budget {
    /// The milliseconds left at `now`.
    pub fn left_ms(&self, now: Instant) -> u64 {
        u64::try_from(self.deadline.saturating_duration_since(now).as_millis()).unwrap_or(u64::MAX)
    }
}

/// What a run may spend and what earlier runs spent (ADR-086): its budget, its concurrency cap
/// (`--jobs`), and the store's history.
#[derive(Debug, Clone, Default)]
pub struct Resources {
    pub budget: Option<Budget>,
    pub jobs: Option<NonZeroUsize>,
    pub history: History,
}

/// A check task offered to the knapsack.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub value: f64,
    /// The check's own cost.
    pub cost_ms: u64,
    /// The producer tasks it needs, as indexes into the producers' costs.
    pub closure: Vec<usize>,
    /// Orders equal ratios ([`tie`]).
    pub tie: u64,
}

/// What the knapsack took, in the order it took it, and what it skipped, each with its
/// marginal cost: its own plus its closure's producer tasks not taken before it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choice {
    pub taken: Vec<(usize, u64)>,
    pub skipped: Vec<(usize, u64)>,
}

/// The greedy knapsack with dependency closure (ADR-086 §5). With a budget, repeatedly takes the
/// candidate with the highest value per marginal cost that fits what is left, until none fits;
/// the rest are skipped with their marginal cost then. Without one, takes every candidate in
/// order. `producers` holds each producer task's cost.
pub fn choose(candidates: &[Candidate], producers: &[u64], budget_ms: Option<u64>) -> Choice {
    let mut bought = vec![false; producers.len()];
    let marginal = |candidate: &Candidate, bought: &[bool]| -> u64 {
        let mut closure = candidate.closure.clone();
        closure.sort_unstable();
        closure.dedup();
        closure
            .iter()
            .filter(|index| !bought[**index])
            .fold(candidate.cost_ms, |sum, index| {
                sum.saturating_add(producers[*index])
            })
    };
    let mut choice = Choice::default();
    let mut open: Vec<usize> = (0..candidates.len()).collect();
    let mut left = budget_ms.unwrap_or(u64::MAX);
    loop {
        let next = open
            .iter()
            .map(|index| (*index, marginal(&candidates[*index], &bought)))
            .filter(|(_, cost)| *cost <= left)
            .reduce(|best, other| {
                let order = if budget_ms.is_none() {
                    std::cmp::Ordering::Less
                } else {
                    better(&candidates[best.0], best.1, &candidates[other.0], other.1)
                };
                if order == std::cmp::Ordering::Greater {
                    other
                } else {
                    best
                }
            });
        let Some((index, cost)) = next else { break };
        for producer in &candidates[index].closure {
            bought[*producer] = true;
        }
        left -= cost;
        choice.taken.push((index, cost));
        open.retain(|open| *open != index);
    }
    choice.skipped = open
        .into_iter()
        .map(|index| (index, marginal(&candidates[index], &bought)))
        .collect();
    choice
}

/// `Less` when `a` (marginal cost `a_ms`) should be taken before `b`: a higher value per
/// millisecond, then the lower tie key.
#[allow(
    clippy::cast_precision_loss,
    reason = "costs in milliseconds stay far below 2^52, where f64 is exact"
)]
fn better(a: &Candidate, a_ms: u64, b: &Candidate, b_ms: u64) -> std::cmp::Ordering {
    let ratio = |candidate: &Candidate, ms: u64| candidate.value / ms.max(1) as f64;
    ratio(b, b_ms)
        .total_cmp(&ratio(a, a_ms))
        .then(a.tie.cmp(&b.tie))
}
