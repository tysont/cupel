// ABOUTME: The crate's one function, which its nightly-only benchmark times; clippy finds nothing to say about
// ABOUTME: it, so the only diagnostic is the benchmark's feature gate.

/// The sum of `values`.
pub fn total(values: &[u64]) -> u64 {
    values.iter().sum()
}

#[cfg(test)]
mod tests {
    #[test]
    fn totals_the_values() {
        assert_eq!(super::total(&[1, 2, 3]), 6);
    }
}
