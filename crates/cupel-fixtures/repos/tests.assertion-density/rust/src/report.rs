//! Reports drawn from a ledger.

/// One line naming `owner` and the balance of `entries`.
pub fn statement(owner: &str, entries: &[i64]) -> String {
    format!("{owner}: {}", crate::balance(entries))
}

/// The largest of `entries`, if there is one.
pub fn largest(entries: &[i64]) -> Option<i64> {
    entries.iter().copied().max()
}
