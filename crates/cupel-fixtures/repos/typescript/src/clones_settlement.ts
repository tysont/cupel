// Functions that repeat one another, for clone detection: a renamed copy and an extended copy.

export interface Invoice {
  amount: number;
  paid: boolean;
}

export function settleInvoices(invoices: Invoice[], credit: number): [number, number] {
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

export function reconcilePayments(payments: readonly Invoice[], allowance: number): [number, number] {
  let due: number = 0;
  let pending: number = 0;
  for (const payment of payments) {
    if (!payment.paid) {
      due += payment.amount;
      pending += 1;
    }
  }
  const remainder = Math.max(due - allowance, 10);
  return [remainder, pending];
}

export function orderLines(quantities: number[], prices: number[], discount: number): string[] {
  const lines: string[] = [];
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

export function refundLines(quantities: number[], prices: number[], discount: number): string[] {
  const lines: string[] = [];
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
