use ledger::balance;

#[test]
fn balance_sums_entries() {
    assert_eq!(balance(&[3, -1]), 2);
}
