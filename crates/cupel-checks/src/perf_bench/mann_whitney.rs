// ABOUTME: The two-sided Mann-Whitney U test perf.bench compares two sides' samples with (benchstat's test): exact over
// ABOUTME: every assignment of midranks while there are at most 100 000, else the normal approximation (ADR-106).
#![allow(
    clippy::cast_precision_loss,
    reason = "sample counts, assignment counts, and rank sums stay far below 2^52, where f64 is exact"
)]

/// The most assignments of ranks to the first sample the exact distribution enumerates.
const EXACT_LIMIT: u64 = 100_000;

/// The two-sided p-value of the Mann-Whitney U test of `a` against `b`: the chance of a rank
/// sum at least as far from its mean as `a`'s if both came from one distribution. Ties share
/// their midrank; when every value is equal the p-value is 1. Either side empty is 1 too.
pub fn p_value(a: &[f64], b: &[f64]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 1.0;
    }
    let ranks = doubled_ranks(a, b);
    if combinations(ranks.len(), a.len()).is_some_and(|count| count <= EXACT_LIMIT) {
        exact(&ranks, a.len())
    } else {
        approximate(&ranks, a, b)
    }
}

/// Twice the midrank of every value of `a`, then `b`, in the pooled sample: integers, so the
/// exact distribution counts rank sums without rounding.
fn doubled_ranks(a: &[f64], b: &[f64]) -> Vec<u64> {
    let pooled: Vec<f64> = a.iter().chain(b).copied().collect();
    let mut order: Vec<usize> = (0..pooled.len()).collect();
    order.sort_by(|&i, &j| pooled[i].total_cmp(&pooled[j]));
    let mut ranks = vec![0; pooled.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start;
        while end + 1 < order.len()
            && pooled[order[end + 1]]
                .total_cmp(&pooled[order[start]])
                .is_eq()
        {
            end += 1;
        }
        // Positions start..=end hold ranks start+1..=end+1; twice their mean is their sum.
        let doubled = (start + end + 2) as u64;
        for &index in &order[start..=end] {
            ranks[index] = doubled;
        }
        start = end + 1;
    }
    ranks
}

/// `n` choose `k`, when it fits in a `u64`.
fn combinations(n: usize, k: usize) -> Option<u64> {
    let k = k.min(n - k) as u64;
    let n = n as u64;
    (0..k).try_fold(1u64, |count, i| {
        count.checked_mul(n - i).map(|product| product / (i + 1))
    })
}

/// The exact p-value: of every way to give `first` of the pooled values to the first sample,
/// the share whose rank sum is at least as far from the mean as the observed one.
fn exact(ranks: &[u64], first: usize) -> f64 {
    let observed: u64 = ranks[..first].iter().sum();
    // The mean rank sum is first * (N + 1) / 2; in doubled ranks, first * (N + 1).
    let mean = (first * (ranks.len() + 1)) as u64;
    let tally = Tally {
        ranks,
        mean,
        far: observed.abs_diff(mean),
    };
    let (hits, all) = tally.count(0, first, 0);
    hits as f64 / all as f64
}

/// Enumerates the ways to choose ranks, counting those whose sum is at least `far` from `mean`.
struct Tally<'a> {
    ranks: &'a [u64],
    mean: u64,
    far: u64,
}

impl Tally<'_> {
    /// (far enough, all) among the ways to choose `left` more ranks from `from` on, the chosen
    /// ones summing to `sum` so far.
    fn count(&self, from: usize, left: usize, sum: u64) -> (u64, u64) {
        if left == 0 {
            return (u64::from(sum.abs_diff(self.mean) >= self.far), 1);
        }
        (from..=self.ranks.len() - left).fold((0, 0), |(hits, all), at| {
            let (more, every) = self.count(at + 1, left - 1, sum + self.ranks[at]);
            (hits + more, all + every)
        })
    }
}

/// The normal approximation of U's distribution, with the tie correction of its variance and a
/// continuity correction of half a unit.
fn approximate(ranks: &[u64], first: &[f64], second: &[f64]) -> f64 {
    let (n1, n2) = (first.len() as f64, second.len() as f64);
    let pooled = n1 + n2;
    let rank_sum = ranks[..first.len()].iter().sum::<u64>() as f64 / 2.0;
    let statistic = rank_sum - n1 * (n1 + 1.0) / 2.0;
    let mean = n1 * n2 / 2.0;
    let variance = n1 * n2 / 12.0 * ((pooled + 1.0) - tie_term(ranks) / (pooled * (pooled - 1.0)));
    if variance <= 0.0 {
        return 1.0;
    }
    let score = ((statistic - mean).abs() - 0.5).max(0.0) / variance.sqrt();
    erfc(score / std::f64::consts::SQRT_2)
}

/// The sum of t³ − t over every group of t tied values.
fn tie_term(ranks: &[u64]) -> f64 {
    let mut sorted = ranks.to_vec();
    sorted.sort_unstable();
    sorted
        .chunk_by(|a, b| a == b)
        .map(|group| {
            let t = group.len() as f64;
            t * t * t - t
        })
        .sum()
}

/// The complementary error function, by the Chebyshev fit of Numerical Recipes (`erfcc`),
/// accurate to 1.2e-7 relative everywhere.
fn erfc(x: f64) -> f64 {
    const COEFFICIENTS: [f64; 10] = [
        -1.265_512_23,
        1.000_023_68,
        0.374_091_96,
        0.096_784_18,
        -0.186_288_06,
        0.278_868_07,
        -1.135_203_98,
        1.488_515_87,
        -0.822_152_23,
        0.170_872_77,
    ];
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let polynomial = COEFFICIENTS
        .iter()
        .rev()
        .fold(0.0, |sum, coefficient| sum * t + coefficient);
    let value = t * (-z * z + polynomial).exp();
    if x >= 0.0 { value } else { 2.0 - value }
}
