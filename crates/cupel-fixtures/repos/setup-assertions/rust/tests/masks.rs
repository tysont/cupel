// ABOUTME: Two tests that open with three assertions each, on different functions: nothing they share is setup.
// ABOUTME: Two tests that arrange the same three values before asserting: duplicated setup.
use bits::{Pair, halfway, lower_mask};

#[test]
fn lower_mask_test() {
    assert_eq!(lower_mask(0), 0b0);
    assert_eq!(lower_mask(1), 0b1);
    assert_eq!(lower_mask(2), 0b11);
    assert_eq!(lower_mask(3), 0b111);
}

#[test]
fn halfway_test() {
    assert_eq!(halfway(0), 0b0);
    assert_eq!(halfway(1), 0b1);
    assert_eq!(halfway(2), 0b10);
    assert_eq!(halfway(3), 0b100);
}

#[test]
fn sum_test() {
    let left = 2;
    let right = 3;
    let pair = Pair { left, right };
    assert_eq!(pair.sum(), 5);
}

#[test]
fn product_test() {
    let left = 2;
    let right = 3;
    let pair = Pair { left, right };
    assert_eq!(pair.product(), 6);
}
