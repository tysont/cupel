// Package tally is a passing test suite for Cupel's runner producers.
package tally

// Total is the sum of values.
func Total(values []int) int {
	sum := 0
	for _, value := range values {
		sum += value
	}
	return sum
}
