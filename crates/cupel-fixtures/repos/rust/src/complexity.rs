//! Ledger helpers of varying shape: each of the first four trips one `static.complexity` limit,
//! and the rest stay within every limit.

/// Labels an amount by the first rule it matches.
pub fn label(amount: i64, limit: i64) -> &'static str {
    let label;
    if amount < 0 {
        label = "refund";
    } else if amount == 0 {
        label = "empty";
    } else if amount > limit * 100 {
        label = "huge";
    } else if amount > limit * 10 {
        label = "large";
    } else if amount > limit {
        label = "over";
    } else if amount == limit {
        label = "at-limit";
    } else if amount > limit / 2 {
        label = "high";
    } else if amount > limit / 4 {
        label = "medium";
    } else if amount > limit / 10 {
        label = "low";
    } else if amount > 1 {
        label = "tiny";
    } else {
        label = "unit";
    }
    label
}

/// Applies positive amounts of non-empty batches until the limit is reached.
pub fn settle(batches: &[Vec<i64>], limit: i64) -> i64 {
    let mut settled = 0;
    for batch in batches {
        if !batch.is_empty() {
            for amount in batch {
                if *amount > 0 {
                    while settled < limit {
                        settled += *amount;
                    }
                }
            }
        }
    }
    settled
}

/// Describes a new account.
pub fn open_account(
    owner: &str,
    currency: &str,
    limit: i64,
    overdraft: bool,
    branch: u32,
    opened: u64,
) -> String {
    format!("{owner}/{currency}/{limit}/{overdraft}/{branch}/{opened}")
}

/// The fixed lines of a printed statement.
pub fn statement_template() -> Vec<&'static str> {
    vec![
        "ACME LEDGER",
        "===========",
        "",
        "Account holder:",
        "Account number:",
        "Branch:",
        "Currency:",
        "Statement period:",
        "",
        "Opening balance",
        "---------------",
        "Balance brought forward:",
        "Pending credits:",
        "Pending debits:",
        "",
        "Credits",
        "-------",
        "Deposits:",
        "Transfers in:",
        "Interest:",
        "Refunds:",
        "Adjustments:",
        "",
        "Debits",
        "------",
        "Withdrawals:",
        "Transfers out:",
        "Card payments:",
        "Direct debits:",
        "Standing orders:",
        "Fees:",
        "Charges:",
        "",
        "Closing balance",
        "---------------",
        "Balance carried forward:",
        "Available balance:",
        "Overdraft limit:",
        "Overdraft used:",
        "",
        "Notices",
        "-------",
        "Rates changed this period.",
        "Fees are listed above.",
        "Keep this statement for your records.",
        "Report errors within thirty days.",
        "",
        "Contact",
        "-------",
        "Telephone:",
        "Email:",
        "Post:",
        "",
        "Opening hours",
        "-------------",
        "Weekdays: 9 to 5",
        "Saturday: 9 to 12",
        "Sunday: closed",
        "",
        "Page 1 of 1",
        "End of statement",
    ]
}

/// The difference between credits and debits.
pub fn net(credits: i64, debits: i64) -> i64 {
    credits - debits
}

/// A printed statement for one account.
pub struct Statement {
    pub owner: String,
}

impl Statement {
    /// One line of the statement.
    pub fn line(&self, amount: i64) -> String {
        format!("{}: {amount}", self.owner)
    }
}

/// Names a month by its number: one match, however many arms it has.
pub fn month(number: u32) -> &'static str {
    match number {
        1 => "jan",
        2 => "feb",
        3 => "mar",
        4 => "apr",
        5 => "may",
        6 => "jun",
        7 => "jul",
        8 => "aug",
        9 => "sep",
        10 => "oct",
        11 => "nov",
        12 => "dec",
        _ => "none",
    }
}
