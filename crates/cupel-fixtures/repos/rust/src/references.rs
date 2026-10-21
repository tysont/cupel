//! Reference cases for `static.dead-code` (ADR-135): an exported rate, one only an import
//! reaches, one only a table that names it reaches, and one nothing names.

/// Rates looked up by the name a caller passes.
const RATES: [(&str, fn(i64) -> i64); 1] = [("tiered", tiered_rate)];

/// Exported, so live by convention though nothing calls it.
pub fn exported_rate(amount: i64) -> i64 {
    amount * 3
}

/// Called only from `rates.rs`, which imports it.
pub(crate) fn imported_rate(amount: i64) -> i64 {
    amount + amount
}

fn tiered_rate(amount: i64) -> i64 {
    if amount > 100 { amount / 3 } else { amount }
}

fn orphan_rate(amount: i64) -> i64 {
    amount - amount / 4
}

/// The rate `kind` names, applied to `amount`.
pub fn rate_named(kind: &str, amount: i64) -> Option<i64> {
    RATES
        .iter()
        .find(|(name, _)| *name == kind)
        .map(|(_, rate)| rate(amount))
}
