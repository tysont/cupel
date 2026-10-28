//! Ledger: three functions, each covered by its own test; `add` by two.

/// The sum of `a` and `b`.
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

/// `value` scaled by `factor`.
pub fn scale(value: i64, factor: i64) -> i64 {
    value * factor
}

/// `value` with its sign flipped.
pub fn negate(value: i64) -> i64 {
    -value
}
