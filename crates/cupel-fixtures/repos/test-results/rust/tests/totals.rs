//! An integration test of the tally fixture.

#[test]
fn an_empty_list_totals_zero() {
    assert_eq!(tally::total(&[]), 0);
}
