//! A large member order: the discount applies.

#[test]
fn discounts_a_large_member_order() {
    assert_eq!(discount::discount(200, true), 190);
}
