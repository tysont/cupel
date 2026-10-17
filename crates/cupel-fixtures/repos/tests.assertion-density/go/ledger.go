// Package ledger keeps signed amounts and the reports drawn from them.
package ledger

// Balance is the sum of entries.
func Balance(entries []int) int {
	total := 0
	for _, entry := range entries {
		total += entry
	}
	return total
}

// Overdrawn reports whether entries sum below zero.
func Overdrawn(entries []int) bool {
	return Balance(entries) < 0
}
