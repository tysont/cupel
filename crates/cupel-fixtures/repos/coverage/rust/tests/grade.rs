//! Tests of the grade fixture: `total` and the passing grades of `classify`.

#[test]
fn totals_a_list() {
    assert_eq!(grade::total(&[1, 2, 3]), 6);
}

#[test]
fn classifies_passing_scores() {
    assert_eq!(grade::classify(95), "A");
    assert_eq!(grade::classify(60), "pass");
}
