// ABOUTME: Properties in a quickcheck! block, each asserting through `check_length`, a helper of this file;
// ABOUTME: each is a test that makes one strong assertion.
use counts::{doubled, evens};
use quickcheck::quickcheck;

fn check_length(found: usize, expected: usize) {
    assert_eq!(found, expected);
}

quickcheck! {
    fn doubling_keeps_the_length(values: Vec<u8>) -> () {
        check_length(doubled(&values).len(), values.len());
    }

    fn evens_of_evens_keep_the_length(values: Vec<u8>) -> () {
        check_length(evens(&evens(&values)).len(), evens(&values).len());
    }

    fn doubling_twice_keeps_the_length(values: Vec<u8>) -> () {
        check_length(doubled(&values).len(), doubled(&values).len());
    }
}
