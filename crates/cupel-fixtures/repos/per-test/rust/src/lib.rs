//! Calc: two functions, each covered by exactly one test, for Cupel's per-test coverage map.

/// The sum of `a` and `b`.
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

/// `value` scaled by `factor`.
pub fn scale(value: i64, factor: i64) -> i64 {
    value * factor
}

#[cfg(test)]
mod tests {
    use super::{add, scale};

    #[test]
    fn adds() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn scales() {
        assert_eq!(scale(2, 3), 6);
    }
}
