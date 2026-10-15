// Package grade is a suite that covers Total fully, Classify partly, and Refund not at all.
package grade

// Total is the sum of values.
func Total(values []int) int {
	sum := 0
	for _, value := range values {
		sum += value
	}
	return sum
}

// Classify is a letter for score; the tests never reach the failing grade.
func Classify(score int) string {
	if score >= 90 {
		return "A"
	}
	if score >= 50 {
		return "pass"
	}
	return "fail"
}

// Refund is what to refund of amount after days; no test calls it.
func Refund(amount int, days int, member bool) int {
	if amount <= 0 {
		return 0
	}
	if days > 30 {
		return 0
	}
	if member && days < 7 {
		return amount
	}
	if member {
		return amount / 2
	}
	return amount / 4
}
