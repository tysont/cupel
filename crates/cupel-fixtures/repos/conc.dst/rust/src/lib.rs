//! A ledger whose deposits read the balance, wait on a slow audit, and write the sum back.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

/// A balance shared by the tasks of one simulated node.
#[derive(Clone, Default)]
pub struct Ledger(Rc<Cell<u64>>);

impl Ledger {
    pub fn balance(&self) -> u64 {
        self.0.get()
    }

    /// Adds `amount` after an audit of `audit` milliseconds: a read, a wait, and a write, so two
    /// deposits whose audits overlap lose one of them.
    pub async fn deposit(&self, amount: u64, audit: u64) {
        let seen = self.0.get();
        println!("deposit {amount} reads {seen}");
        madsim::time::sleep(Duration::from_millis(audit)).await;
        self.0.set(seen + amount);
        println!("deposit {amount} writes {}", seen + amount);
    }

    /// Takes `amount` out, refusing to overdraw the balance.
    pub fn withdraw(&self, amount: u64) {
        let balance = self.0.get();
        assert!(amount <= balance, "overdrawn: {amount} from {balance}");
        self.0.set(balance - amount);
    }
}
