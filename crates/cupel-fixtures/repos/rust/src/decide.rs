pub fn classify(amount: i64, limit: i64) -> &'static str {
    if amount < 0 {
        "debit"
    } else if amount > limit && limit > 0 {
        "large"
    } else {
        "credit"
    }
}

pub fn first_overdraft(balances: &[i64]) -> Option<usize> {
    for (index, balance) in balances.iter().enumerate() {
        if *balance < 0 {
            return Some(index);
        }
    }
    None
}

pub fn parse_cents(text: &str) -> Result<i64, std::num::ParseIntError> {
    let cents = text.trim().parse::<i64>()?;
    Ok(cents)
}

pub fn describe(code: u8) -> &'static str {
    match code {
        0 => "ok",
        1 | 2 => "retry",
        _ => "fail",
    }
}
