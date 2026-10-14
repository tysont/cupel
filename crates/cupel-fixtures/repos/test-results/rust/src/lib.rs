//! Tally: a passing test suite for Cupel's runner producers, with one ignored test.

/// The sum of `values`.
pub fn total(values: &[i64]) -> i64 {
    values.iter().sum()
}

#[cfg(test)]
mod tests {
    use super::total;

    #[test]
    fn totals_a_list() {
        assert_eq!(total(&[1, 2, 3]), 6);
    }

    #[test]
    #[ignore = "kept to show a skipped test"]
    fn totals_a_huge_list() {
        assert_eq!(total(&[1; 1000]), 1000);
    }
}
