//! Ledger helpers in each `static.crap` band when no test covers them: `overdue` is `crap.high`
//! (cyclomatic 6, CRAP 42), `clamp` is `crap.watch` (cyclomatic 3, CRAP 12), and `to_cents` is
//! clean.

/// Counts entries more than thirty days late and past the grace period; reversals (negative
/// days) and voided entries (zero days) each cancel one.
pub fn overdue(days: &[i64], grace: i64) -> i64 {
    let mut count = 0;
    for day in days {
        if *day > 30 && *day > grace {
            count += 1;
        } else if *day < 0 {
            count -= 1;
        } else if *day == 0 {
            count -= 1;
        }
    }
    count
}

/// Keeps an amount between low and high.
pub fn clamp(amount: i64, low: i64, high: i64) -> i64 {
    if amount < low {
        return low;
    }
    if amount > high {
        return high;
    }
    amount
}

/// Converts whole units to cents.
pub fn to_cents(units: i64) -> i64 {
    units * 100
}
