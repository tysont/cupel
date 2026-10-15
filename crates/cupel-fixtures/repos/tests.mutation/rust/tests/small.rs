//! A small order: no discount; never the boundary of exactly 100.

#[test]
fn keeps_a_small_order() {
    assert_eq!(discount::discount(50, true), 50);
}
