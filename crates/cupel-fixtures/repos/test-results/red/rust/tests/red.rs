//! The red-baseline overlay: one failing test.

#[test]
fn reports_a_wrong_total() {
    println!("totalling 2 and 2");
    assert_eq!(tally::total(&[2, 2]), 5);
}
