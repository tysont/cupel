package ledger

// Functions that repeat one another, for clone detection: renamed, extended, and exact copies.

type Invoice struct {
	Amount int64
	Paid   bool
}

func settleInvoices(invoices []Invoice, credit int64) (int64, int) {
	owed := int64(0)
	open := 0
	for _, invoice := range invoices {
		if !invoice.Paid {
			owed += invoice.Amount
			open++
		}
	}
	balance := max(owed-credit, 0)
	return balance, open
}

func reconcilePayments(payments []Invoice, allowance int64) (int64, int) {
	due := int64(0)
	pending := 0
	for _, payment := range payments {
		if !payment.Paid {
			due += payment.Amount
			pending++
		}
	}
	remainder := max(due-allowance, 10)
	return remainder, pending
}

func orderLines(quantities []int64, prices []int64, discount int64) []int64 {
	lines := make([]int64, 0, len(prices))
	total := int64(0)
	count := int64(0)
	for index, quantity := range quantities {
		cost := quantity*prices[index] - discount
		total += cost
		count += quantity
		lines = append(lines, cost)
	}
	average := total / max(count, 1)
	lines = append(lines, total)
	lines = append(lines, average)
	return lines
}

func refundLines(quantities []int64, prices []int64, discount int64) []int64 {
	lines := make([]int64, 0, len(prices))
	total := int64(0)
	count := int64(0)
	for index, quantity := range quantities {
		cost := quantity*prices[index] - discount
		total += cost
		count += quantity
		lines = append(lines, cost)
	}
	lines = append(lines, count)
	average := total / max(count, 1)
	lines = append(lines, total)
	lines = append(lines, average)
	return lines
}

func runningRefunds(refunds []int64, start int64) []int64 {
	totals := make([]int64, 0, len(refunds))
	spent := start
	for _, refund := range refunds {
		spent += refund
		totals = append(totals, spent)
	}
	return append(totals, spent-start)
}

func runningRefundsAgain(refunds []int64, start int64) []int64 {
	// The same body as runningRefunds, laid out differently.
	totals := make([]int64, 0, len(refunds))
	spent := start

	for _, refund := range refunds {
		spent += refund // the total so far
		totals = append(totals, spent)
	}

	return append(totals, spent-start)
}
