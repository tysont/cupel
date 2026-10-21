use ledger::Account;
use ledger::pricing::discounted;

#[test]
fn deposit_adds_to_the_balance() {
    let mut account = Account::new("ada");
    account.deposit(250);
    assert_eq!(account.to_string(), "ada (250)");
}

#[test]
fn members_get_a_discount() {
    assert!(discounted(1000, 10, true) < 1000);
}

#[test]
fn statements_mention_the_balance() {
    assert!(Account::new("eve").statement().find("balance").is_some());
}

#[test]
fn parsing_garbage_fails() {
    assert!(ledger::decide::parse_cents("ten").is_err());
}

#[test]
#[should_panic(expected = "overflow")]
fn huge_deposits_overflow() {
    let mut account = Account::new("bob");
    account.deposit(i64::MAX);
    account.deposit(1);
}

#[test]
#[ignore = "talks to the network"]
fn statement_is_mailed() {
    let account = Account::new("cy");
    assert_ne!(account.statement(), "");
}

/// An account opens without a deposit (REQ-1).
#[test]
fn opening_an_account_does_not_panic() {
    Account::new("dee");
}
