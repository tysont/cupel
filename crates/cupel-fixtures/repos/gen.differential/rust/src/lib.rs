/// A quarter of `cents`, rounded down.
pub fn quarter(cents: i64) -> i64 {
    cents.div_euclid(4)
}
