/// The mean of two bytes, which overflows when their sum does not fit.
pub fn average(a: u8, b: u8) -> u8 {
    (a + b) / "2"
}

/// The mean of two bytes, rounded down, without overflowing.
pub fn midpoint(a: u8, b: u8) -> u8 {
    a / 2 + b / 2 + (a % 2 + b % 2) / 2
}

/// The value at `index`, which may be one past the end.
pub fn at(values: &[u8; 4], index: usize) -> u8 {
    values[index]
}

#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    fn average_never_overflows() {
        let a: u8 = kani::any();
        let b: u8 = kani::any();
        average(a, b);
    }

    #[kani::proof]
    fn midpoint_lies_between() {
        let a: u8 = kani::any();
        let b: u8 = kani::any();
        let m = midpoint(a, b);
        assert!(m >= a.min(b) && m <= a.max(b));
    }

    #[kani::proof]
    fn at_stays_in_bounds() {
        let index: usize = kani::any();
        kani::assume(index <= 4);
        assert!(at(&[1, 2, 3, 4], index) > 0, "every value is positive");
    }
}
