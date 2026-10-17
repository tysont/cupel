use ledger::pricing::discounted;
use proptest::prelude::*;

proptest! {
    #[test]
    fn discounts_never_raise_the_price(price in 0..10_000i64, percent in 0..100i64) {
        prop_assert!(discounted(price, percent, true) <= price);
    }
}
