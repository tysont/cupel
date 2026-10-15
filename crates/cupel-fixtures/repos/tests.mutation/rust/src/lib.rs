//! Discount: a weak suite never tries a total of exactly 100, and never calls `refund`.

/// Ten off a member's order of 100 or more.
pub fn discount(total: i64, member: bool) -> i64 {
    if total >= 100 && member {
        return total - 10;
    }
    total * 1
}

/// What a returned order gives back.
pub fn refund(total: i64) -> i64 {
    total - 5
}
