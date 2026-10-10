//! A small ledger: the Rust fixture repository for Cupel's language layer.

pub mod account;
pub mod report;

pub use account::Account;

/// Sums every entry.
pub fn balance(entries: &[i64]) -> i64 {
    entries.iter().sum()
}

pub(crate) fn audit_limit() -> i64 {
    10_000
}

fn scale(amount: i64, factor: i64) -> i64 {
    amount * factor
}
