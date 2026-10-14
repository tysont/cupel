// Functions that repeat one another, for clone detection: a renamed copy and an extended copy.

export function settleInvoices(invoices, credit) {
  let owed = 0;
  let open = 0;
  for (const invoice of invoices) {
    if (!invoice.paid) {
      owed += invoice.amount;
      open += 1;
    }
  }
  const balance = Math.max(owed - credit, 0);
  return [balance, open];
}

export function reconcilePayments(payments, allowance) {
  let due = 0;
  let pending = 0;
  for (const payment of payments) {
    if (!payment.paid) {
      due += payment.amount;
      pending += 1;
    }
  }
  const remainder = Math.max(due - allowance, 10);
  return [remainder, pending];
}

export function orderLines(quantities, prices, discount) {
  const lines = [];
  let total = 0;
  let count = 0;
  quantities.forEach((quantity, index) => {
    const cost = quantity * prices[index] - discount;
    total += cost;
    count += quantity;
    lines.push(`${quantity} x ${prices[index]} = ${cost}`);
  });
  const average = total / Math.max(count, 1);
  lines.push(`total ${total}`);
  lines.push(`average ${average}`);
  return lines;
}

export function refundLines(quantities, prices, discount) {
  const lines = [];
  let total = 0;
  let count = 0;
  quantities.forEach((quantity, index) => {
    const cost = quantity * prices[index] - discount;
    total += cost;
    count += quantity;
    lines.push(`${quantity} x ${prices[index]} = ${cost}`);
  });
  lines.sort();
  const average = total / Math.max(count, 1);
  lines.push(`total ${total}`);
  lines.push(`average ${average}`);
  return lines;
}
