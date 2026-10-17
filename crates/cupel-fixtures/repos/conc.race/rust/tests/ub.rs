#[test]
fn reads_the_last_value() {
    let _ = tally::last(&[1, 2, 3, 4]);
}
