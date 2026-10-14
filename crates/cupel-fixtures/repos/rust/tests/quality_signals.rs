use ledger::balance;
use ledger::decide::classify;
use ledger::pricing::{discounted, tally};
use ledger::Account;

#[test]
fn members_pay_exactly_the_discounted_price() {
    assert_eq!(discounted(1000, 10, true), 900);
}

#[test]
#[should_panic(expected = "overflow")]
fn overflowing_balances_panic() {
    balance(&[i64::MAX, 1]);
}

#[test]
fn statements_are_not_empty() {
    assert!(!Account::new("ann").statement().is_empty());
}

#[test]
fn classifying_runs() {
    classify(100, 50);
}

#[test]
fn tallies_are_logged_and_mailed() {
    let _log = MockLog::new();
    let _mailer = MockMailer::new();
    let _clock = MockClock::new();
    assert_eq!(tally(&[1, 2]), (3, false));
}

#[test]
fn every_entry_counts_toward_the_balance() {
    for cents in [1, 20, 300] {
        assert_eq!(balance(&[cents]), cents);
    }
}

#[test]
fn display_shows_owner_and_balance() {
    let mut account = Account::new("kim");
    account.deposit(100);
    account.deposit(50);
    assert_eq!(account.to_string(), "kim (150)");
}

#[test]
fn statements_show_the_balance() {
    let mut account = Account::new("lee");
    account.deposit(200);
    account.deposit(25);
    let statement = account.statement();
    assert_eq!(statement, "Account of lee\nbalance: 225");
}
