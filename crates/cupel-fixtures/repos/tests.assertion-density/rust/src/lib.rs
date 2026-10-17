//! A ledger of signed amounts and the reports drawn from it.
pub mod report;

/// The sum of `entries`.
pub fn balance(entries: &[i64]) -> i64 {
    entries.iter().sum()
}

/// Whether `entries` sum below zero.
pub fn overdrawn(entries: &[i64]) -> bool {
    balance(entries) < 0
}
