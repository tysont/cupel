//! Tests of the ledger fixture: `scales` alone covers `scale`.

#[test]
fn adds() {
    assert_eq!(ledger::add(2, 3), 5);
}

#[test]
fn adds_zero() {
    assert_eq!(ledger::add(7, 0), 7);
}

#[test]
fn scales() {
    assert_eq!(ledger::scale(2, 3), 6);
}

#[test]
fn negates() {
    assert_eq!(ledger::negate(4), -4);
}
