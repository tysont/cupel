/// The sum of `values`, as proofs/tally.dfy proves `Total` computes it.
pub fn total(values: &[i64]) -> i64 {
    values.iter().sum()
}
