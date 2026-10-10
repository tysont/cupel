//! A small ledger: the Rust fixture repository for Cupel's language layer.

pub fn balance(entries: &[i64]) -> i64 {
    entries.iter().sum()
}
