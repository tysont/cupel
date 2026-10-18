package tally

// Total is the sum of values, as proofs/tally.dfy proves `Total` computes it.
func Total(values []int) int {
	total := 0
	for _, value := range values {
		total += value
	}
	return total
}
