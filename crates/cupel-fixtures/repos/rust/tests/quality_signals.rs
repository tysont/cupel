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
fn computed_entries_count_toward_the_balance() {
    for cents in (1..=3).map(|n| n * 100) {
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

#[test]
fn a_comparison_inside_assert_pins_the_balance() {
    assert!(balance(&[1, 2]) == 3);
}

fn assert_balanced(entries: &[i64], expected: i64) {
    assert_eq!(balance(entries), expected);
}

#[test]
fn an_assertion_helper_is_an_assertion() {
    assert_balanced(&[2, 3], 5);
}

const LISTED: [i64; 3] = [5, 50, 500];

#[test]
fn every_listed_entry_counts_toward_the_balance() {
    for cents in LISTED {
        assert_eq!(balance(&[cents]), cents);
    }
}

fn assert_balance_of<T: Into<i64> + Copy>(entries: &[T], expected: i64) {
    let cents: Vec<i64> = entries.iter().map(|&entry| entry.into()).collect();
    assert_eq!(balance(&cents), expected);
}

#[test]
fn an_assertion_helper_called_with_a_turbofish_is_an_assertion() {
    assert_balance_of::<i32>(&[2, 3], 5);
}

fn expect_balance(entries: &[i64], expected: i64) {
    assert_eq!(balance(entries), expected);
}

#[test]
fn a_helper_of_this_file_that_asserts_is_an_assertion() {
    expect_balance(&[4, 5], 9);
}

macro_rules! with_entries {
    ($entries:expr, |$name:ident| $body:block) => {{
        let $name: &[i64] = $entries;
        $body
    }};
}

#[test]
fn an_assertion_in_a_macro_argument_is_an_assertion() {
    with_entries!(&[1, 1], |entries| {
        assert_eq!(balance(entries), 2);
    });
}
