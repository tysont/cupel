// ABOUTME: Three counting functions: one only its doctest calls, one a unit test calls, and one nothing calls, so
// ABOUTME: coverage without doctests reports the first as doctest-only and the last as uncovered.

/// The sum of two counts.
///
/// ```
/// assert_eq!(tally::add(2, 3), 5);
/// ```
pub fn add(a: u64, b: u64) -> u64 {
    a + b
}

/// Twice a count.
pub fn double(count: u64) -> u64 {
    count * 2
}

/// Half a count, rounded down.
pub fn halve(count: u64) -> u64 {
    count / 2
}

#[cfg(test)]
mod tests {
    #[test]
    fn doubles_a_count() {
        assert_eq!(super::double(2), 4);
    }
}
