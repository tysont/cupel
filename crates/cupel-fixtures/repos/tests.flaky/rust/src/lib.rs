//! One balance in a static, which the tests share.
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

    // Passes only while no test before it has deposited: order-dependent.
    #[test]
    fn balance_starts_at_zero() {
        assert_eq!(BALANCE.load(Ordering::SeqCst), 0);
    }

    // Leaves the shared balance changed for every test after it. The pause keeps it from
    // depositing before the first test reads the balance when the two run in parallel.
    #[test]
    fn deposit_changes_balance() {
        std::thread::sleep(std::time::Duration::from_millis(300));
        deposit(5);
        assert!(BALANCE.load(Ordering::SeqCst) >= 5);
    }
}
