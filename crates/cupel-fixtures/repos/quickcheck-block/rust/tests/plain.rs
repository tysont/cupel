// ABOUTME: Example tests of the same helpers, one assertion each, which set the repository's density the
// ABOUTME: properties' file is compared with.
use counts::{doubled, evens};

#[test]
fn doubled_doubles() {
    assert_eq!(doubled(&[1, 2]), [2, 4]);
}

#[test]
fn evens_keeps_evens() {
    assert_eq!(evens(&[1, 2, 4]), [2, 4]);
}

#[test]
fn evens_of_nothing_is_nothing() {
    assert_eq!(evens(&[]), Vec::<u8>::new());
}
