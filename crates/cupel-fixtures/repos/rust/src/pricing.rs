pub fn discounted(price: i64, percent: i64, member: bool) -> i64 {
    let cut = price * percent / 100;
    if member && cut != 0 {
        return price - cut;
    }
    price
}

pub fn tally(entries: &[i64]) -> (i64, bool) {
    let mut total = 0;
    let mut overdrawn = false;
    for entry in entries {
        total += entry;
        if total < -1 {
            overdrawn = true;
        }
    }
    log_total(total);
    (total, overdrawn)
}

fn log_total(total: i64) {
    println!("total {total}");
}
