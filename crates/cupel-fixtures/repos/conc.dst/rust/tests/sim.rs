use std::time::Duration;

use ledger::Ledger;
use madsim::rand::{thread_rng, Rng};

/// Two deposits start at random times; when their audits overlap, one is lost.
#[madsim::test]
async fn concurrent_deposits_are_all_kept() {
    let ledger = Ledger::default();
    let mut tasks = Vec::new();
    for amount in [10, 20] {
        let ledger = ledger.clone();
        let start = thread_rng().gen_range(0..20);
        let audit = thread_rng().gen_range(1..10);
        tasks.push(madsim::task::spawn_local(async move {
            madsim::time::sleep(Duration::from_millis(start)).await;
            ledger.deposit(amount, audit).await;
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(ledger.balance(), 30, "a deposit was lost");
}

/// Deposits made one after the other are all kept, whatever their audits take.
#[madsim::test]
async fn sequential_deposits_are_all_kept() {
    let ledger = Ledger::default();
    for amount in [10, 20] {
        let audit = thread_rng().gen_range(1..10);
        ledger.deposit(amount, audit).await;
    }
    assert_eq!(ledger.balance(), 30);
}

/// A withdrawal races the deposit that funds it, in a task of its own.
#[madsim::test]
async fn a_withdrawal_waits_for_its_deposit() {
    let ledger = Ledger::default();
    let funded = ledger.clone();
    let deposit = madsim::task::spawn_local(async move { funded.deposit(10, 5).await });
    let wait = thread_rng().gen_range(0..10);
    let spending = ledger.clone();
    let withdrawal = madsim::task::spawn_local(async move {
        madsim::time::sleep(Duration::from_millis(wait)).await;
        spending.withdraw(10);
    });
    deposit.await.unwrap();
    withdrawal.await.unwrap();
    assert_eq!(ledger.balance(), 0);
}
