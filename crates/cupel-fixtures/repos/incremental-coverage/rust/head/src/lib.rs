//! Ledger: three functions, each covered by its own test; `add` by two.

/// The sum of `a` and `b`.
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

/// `value` scaled by `factor`.
pub fn scale(value: i64, factor: i64) -> i64 {
    // The product; a change inside one function, as the inner loop's own change is.
    value * factor
}

/// `value` with its sign flipped.
pub fn negate(value: i64) -> i64 {
    -value
}
