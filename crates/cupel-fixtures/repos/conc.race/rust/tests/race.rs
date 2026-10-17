#[test]
fn publishes() {
    assert!(tally::publish(2) <= 2);
}
