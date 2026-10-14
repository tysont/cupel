package ledger

// Ledger helpers in each static.crap band when no test covers them: Overdue is crap.high
// (cyclomatic 6, CRAP 42), Clamp is crap.watch (cyclomatic 3, CRAP 12), and ToCents is clean.

// Overdue counts entries more than thirty days late and past the grace period; reversals
// (negative days) and voided entries (zero days) each cancel one.
func Overdue(days []int64, grace int64) int {
	count := 0
	for _, day := range days {
		if day > 30 && day > grace {
			count++
		} else if day < 0 {
			count--
		} else if day == 0 {
			count--
		}
	}
	return count
}

// Clamp keeps an amount between low and high.
func Clamp(amount, low, high int64) int64 {
	if amount < low {
		return low
	}
	if amount > high {
		return high
	}
	return amount
}

// ToCents converts whole units to cents.
func ToCents(units int64) int64 {
	return units * 100
}
