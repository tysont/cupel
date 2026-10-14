//! Functions that repeat one another, for clone detection: a renamed copy and an extended copy.

pub struct Invoice {
    pub amount: i64,
    pub paid: bool,
}

pub fn settle_invoices(invoices: &[Invoice], credit: i64) -> (i64, usize) {
    let mut owed = 0;
    let mut open = 0;
    for invoice in invoices {
        if !invoice.paid {
            owed += invoice.amount;
            open += 1;
        }
    }
    let balance = (owed - credit).max(0);
    (balance, open)
}

pub fn reconcile_payments(payments: &[Invoice], allowance: i64) -> (i64, usize) {
    let mut due = 0;
    let mut pending = 0;
    for payment in payments {
        if !payment.paid {
            due += payment.amount;
            pending += 1;
        }
    }
    let remainder = (due - allowance).max(10);
    (remainder, pending)
}

pub fn order_lines(quantities: &[i64], prices: &[i64], discount: i64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut total = 0;
    let mut count = 0;
    for (quantity, price) in quantities.iter().zip(prices) {
        let cost = quantity * price - discount;
        total += cost;
        count += quantity;
        lines.push(format!("{quantity} x {price} = {cost}"));
    }
    let average = total / count.max(1);
    lines.push(format!("total {total}"));
    lines.push(format!("average {average}"));
    lines
}

pub fn refund_lines(quantities: &[i64], prices: &[i64], discount: i64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut total = 0;
    let mut count = 0;
    for (quantity, price) in quantities.iter().zip(prices) {
        let cost = quantity * price - discount;
        total += cost;
        count += quantity;
        lines.push(format!("{quantity} x {price} = {cost}"));
    }
    lines.sort();
    let average = total / count.max(1);
    lines.push(format!("total {total}"));
    lines.push(format!("average {average}"));
    lines
}
