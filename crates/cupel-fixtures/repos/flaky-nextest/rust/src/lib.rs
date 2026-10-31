//! One balance in a static, which the tests would share in one process.
use std::sync::atomic::{AtomicI64, Ordering};

/// The running total of every deposit.
pub static BALANCE: AtomicI64 = AtomicI64::new(0);

/// Adds `amount` to the balance.
pub fn deposit(amount: i64) {
    BALANCE.fetch_add(amount, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Order-dependent in one process; nextest gives it a process of its own.
    #[test]
    fn balance_starts_at_zero() {
        assert_eq!(BALANCE.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn deposit_changes_balance() {
        deposit(5);
        assert_eq!(BALANCE.load(Ordering::SeqCst), 5);
    }
}
