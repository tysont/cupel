//! Charges computed from the reference cases' rates.
use crate::references::imported_rate;

/// Doubles `amount` through the imported rate.
pub fn doubled(amount: i64) -> i64 {
    imported_rate(amount)
}
