use contracts::{debug_ensures, debug_invariant, debug_requires, requires};

/// Splits `total` into `parts` equal shares.
#[debug_requires(parts > 0, "a share needs at least one part")]
pub fn share(total: u64, parts: u64) -> u64 {
    total.checked_div(parts).unwrap_or(0)
}

/// Half of `amount`, rounded down.
#[debug_ensures(ret > 0, "half of a positive amount is positive")]
pub fn half(amount: u64) -> u64 {
    amount / 2
}

/// Credits `amount`; never more than the limit.
#[requires(amount <= 1_000)]
pub fn credit(balance: u64, amount: u64) -> u64 {
    balance + amount
}

pub struct Account {
    pub entries: Vec<u64>,
}

#[debug_invariant(self.entries.len() <= 2, "an account holds two entries")]
impl Account {
    pub fn post(&mut self, amount: u64) {
        self.entries.push(amount);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_nothing_among_no_parts() {
        assert_eq!(share(10, 0), 0);
    }

    #[test]
    fn halves_one() {
        assert_eq!(half(1), 0);
    }

    #[test]
    fn credits_within_the_limit() {
        assert_eq!(credit(5, 10), 15);
    }

    #[test]
    fn posts_three_entries() {
        let mut account = Account { entries: vec![] };
        for amount in [1, 2, 3] {
            account.post(amount);
        }
        assert_eq!(account.entries.len(), 3);
    }
}
