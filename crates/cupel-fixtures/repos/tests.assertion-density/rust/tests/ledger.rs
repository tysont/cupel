//! Dense tests: each pins every outcome it exercises.
use ledger::{balance, overdrawn};

#[test]
fn deposits_and_withdrawals_add_up() {
    assert_eq!(balance(&[]), 0);
    assert_eq!(balance(&[5]), 5);
    assert_eq!(balance(&[5, -3]), 2);
    assert_eq!(balance(&[-3, -4]), -7);
}

#[test]
fn a_negative_balance_is_overdrawn() {
    assert!(overdrawn(&[-1]));
    assert!(overdrawn(&[5, -6]));
    assert!(!overdrawn(&[]));
    assert!(!overdrawn(&[5, -5]));
}

#[test]
fn order_does_not_change_the_balance() {
    assert_eq!(balance(&[1, 2, 3]), 6);
    assert_eq!(balance(&[3, 2, 1]), 6);
    assert_eq!(balance(&[2, -2, 1]), 1);
    assert_eq!(balance(&[-2, 1, 2]), 1);
}
