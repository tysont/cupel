use proptest::prelude::*;
use units::digits::digits;

proptest! {
    #[test]
    fn digits_counts_decimal_places(n in 0u32..110_000) {
        prop_assert_eq!(digits(n), n.to_string().len() as u32);
    }
}

fn at_most_ten(n: u32) -> bool {
    digits(n) <= 10
}

#[test]
fn digit_counts_stay_within_ten() {
    quickcheck::quickcheck(at_most_ten as fn(u32) -> bool);
}
