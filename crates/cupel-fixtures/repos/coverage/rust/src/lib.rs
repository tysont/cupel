//! Grade: a suite that covers `total` fully, `classify` partly, and `refund` not at all.

/// The sum of `values`.
pub fn total(values: &[i64]) -> i64 {
    let mut sum = 0;
    for value in values {
        sum += value;
    }
    sum
}

/// A letter for `score`; the tests never reach the failing grade.
pub fn classify(score: i64) -> &'static str {
    if score >= 90 {
        return "A";
    }
    if score >= 50 {
        return "pass";
    }
    "fail"
}

/// What to refund of `amount` after `days`; no test calls it.
pub fn refund(amount: i64, days: i64, member: bool) -> i64 {
    if amount <= 0 {
        return 0;
    }
    if days > 30 {
        return 0;
    }
    if member && days < 7 {
        return amount;
    }
    if member {
        return amount / 2;
    }
    amount / 4
}
