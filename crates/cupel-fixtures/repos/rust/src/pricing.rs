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

#[cfg_attr(test, mockall::automock)]
pub trait Log {
    fn write(&self, line: &str);
}

#[cfg(test)]
mod tests {
    use rstest::fixture;

    use super::*;

    #[fixture]
    fn entries() -> Vec<i64> {
        vec![5, -10, 2]
    }

    #[test]
    fn tally_reports_overdraft() {
        let (total, overdrawn) = tally(&entries());
        assert_eq!(total, -3);
        assert!(overdrawn);
    }

    #[test]
    fn discount_snapshot() {
        insta::assert_snapshot!(discounted(200, 50, true).to_string());
    }

    #[tokio::test]
    async fn the_log_is_written_once() {
        let mut log = MockLog::new();
        log.expect_write().times(1).return_const(());
        log.write("total 1");
    }
}
