//! What the shop charges.

#[test]
fn a_small_order_is_charged_its_fee() {
    assert_eq!(shop::charged(10), 15);
}
