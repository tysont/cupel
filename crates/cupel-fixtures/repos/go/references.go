package ledger

// Reference cases for static.dead-code (ADR-135): an exported rate, one only another file of
// the package calls, one only a table that names it reaches, and one nothing names.

var rates = map[string]func(int64) int64{"tiered": tieredRate}

// ExportedRate is exported, so live by convention though nothing calls it.
func ExportedRate(amount int64) int64 {
	return amount * 3
}

func importedRate(amount int64) int64 {
	return amount + amount
}

func tieredRate(amount int64) int64 {
	if amount > 100 {
		return amount / 3
	}
	return amount
}

func orphanRate(amount int64) int64 {
	return amount - amount/4
}

// RateNamed applies the rate kind names to amount.
func RateNamed(kind string, amount int64) int64 {
	return rates[kind](amount)
}
