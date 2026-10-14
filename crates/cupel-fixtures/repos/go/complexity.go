package ledger

import "fmt"

// Label names an amount by the first rule it matches.
func Label(amount, limit int64) string {
	label := ""
	if amount < 0 {
		label = "refund"
	} else if amount == 0 {
		label = "empty"
	} else if amount > limit*100 {
		label = "huge"
	} else if amount > limit*10 {
		label = "large"
	} else if amount > limit {
		label = "over"
	} else if amount == limit {
		label = "at-limit"
	} else if amount > limit/2 {
		label = "high"
	} else if amount > limit/4 {
		label = "medium"
	} else if amount > limit/10 {
		label = "low"
	} else if amount > 1 {
		label = "tiny"
	} else {
		label = "unit"
	}
	return label
}

// Settle applies positive amounts of non-empty batches until the limit is reached.
func Settle(batches [][]int64, limit int64) int64 {
	var settled int64
	for _, batch := range batches {
		if len(batch) > 0 {
			for _, amount := range batch {
				if amount > 0 {
					for settled < limit {
						settled += amount
					}
				}
			}
		}
	}
	return settled
}

// OpenAccount describes a new account.
func OpenAccount(owner, currency string, limit int64, overdraft bool, branch uint32, opened uint64) string {
	return fmt.Sprintf("%s/%s/%d/%t/%d/%d", owner, currency, limit, overdraft, branch, opened)
}

// StatementTemplate is the fixed lines of a printed statement.
func StatementTemplate() []string {
	return []string{
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
	}
}

// Net is the difference between credits and debits.
func Net(credits, debits int64) int64 {
	return credits - debits
}

// Statement is a printed statement for one account.
type Statement struct {
	Owner string
}

// Line is one line of the statement.
func (s Statement) Line(amount int64) string {
	return fmt.Sprintf("%s: %d", s.Owner, amount)
}

// Month names a month by its number: one switch, however many cases it has.
func Month(number int) string {
	switch number {
	case 1:
		return "jan"
	case 2:
		return "feb"
	case 3:
		return "mar"
	case 4:
		return "apr"
	case 5:
		return "may"
	case 6:
		return "jun"
	case 7:
		return "jul"
	case 8:
		return "aug"
	case 9:
		return "sep"
	case 10:
		return "oct"
	case 11:
		return "nov"
	case 12:
		return "dec"
	default:
		return "none"
	}
}
