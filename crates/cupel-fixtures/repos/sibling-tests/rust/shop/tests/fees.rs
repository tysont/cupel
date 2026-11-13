//! The fee `lib-a` charges, tested from the sibling crate.

#[test]
fn small_orders_pay_the_fee() {
    assert_eq!(lib_a::fee(99), 5);
}

#[test]
fn large_orders_ship_free() {
    assert_eq!(lib_a::fee(100), 0);
}
