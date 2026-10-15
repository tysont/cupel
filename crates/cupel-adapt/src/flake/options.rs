// ABOUTME: What one rerun varies (ADR-078): `Planned` (order, seed, parallelism, explicit order) and the
// ABOUTME: schedule of reruns derived from the run's seed alone, so `--seed` repeats every rerun.
use cupel_core::{RunOptions, RunOrder};

/// How one rerun runs the suite: `explicit` becomes a selection in order, and the rest the
/// runner's [`RunOptions`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub order: RunOrder,
    /// Handed to the runner: its shuffle seed and, where it has one, its hash seed.
    pub seed: u64,
    /// Workers or threads; `None` for the runner's default.
    pub parallelism: Option<u32>,
    /// For a `Reversed` rerun, the test IDs in the order to run them; empty otherwise.
    pub explicit: Vec<String>,
}

/// The default number of reruns: the design's `tests.flaky ×3` at `standard` (§5.8).
pub const DEFAULT_RUNS: u32 = 3;

/// The reruns of a run with `seed`: the first two in the declared order serially (only the seed
/// differs, so a disagreement is chance), the third reversed (a shuffle where the runner takes
/// no explicit order, [`Planned::explicit`] filled in once the first rerun reported its
/// order), the rest seeded shuffles alternately at the runner's default parallelism and
/// serially.
pub fn schedule(seed: u64, runs: u32) -> Vec<Planned> {
    (0..runs)
        .map(|index| Planned {
            order: match index {
                0 | 1 => RunOrder::Declared,
                2 => RunOrder::Reversed,
                _ => RunOrder::Shuffled,
            },
            seed: rerun_seed(seed, index),
            parallelism: if index >= 3 && index % 2 == 1 {
                None
            } else {
                Some(1)
            },
            explicit: Vec::new(),
        })
        .collect()
}

/// Rerun `index`'s seed: `splitmix64(seed + index)`.
pub fn rerun_seed(seed: u64, index: u32) -> u64 {
    let mut z = seed
        .wrapping_add(u64::from(index))
        .wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

impl Planned {
    /// This rerun as it can run with a runner that does (`explicit_order`) or does not take an
    /// explicit order, given the order the first rerun ran in: reversed from it, or a shuffle.
    #[must_use]
    pub fn resolved(mut self, explicit_order: bool, first: Option<&[String]>) -> Self {
        if self.order == RunOrder::Reversed {
            match first.filter(|_| explicit_order) {
                Some(first) => self.explicit = first.iter().rev().cloned().collect(),
                None => self.order = RunOrder::Shuffled,
            }
        }
        self
    }

    /// What the runner is asked for: this rerun's order, seed, and parallelism.
    pub fn options(&self) -> RunOptions {
        RunOptions {
            order: self.order,
            seed: Some(self.seed),
            parallelism: self.parallelism,
            ..RunOptions::default()
        }
    }

    /// Whether the rerun runs one test at a time.
    pub fn serial(&self) -> bool {
        self.parallelism == Some(1)
    }
}
