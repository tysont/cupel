/// The number of decimal digits in `n`. The loop stops at five digits, so six-digit numbers are
/// miscounted: the bug the property finds once it runs enough cases.
pub fn digits(n: u32) -> u32 {
    let mut count = 1;
    let mut limit = 10;
    while n >= limit && count < 5 {
        count += 1;
        limit *= 10;
    }
    count
}
