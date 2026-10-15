// Package discount: a weak suite never tries a total of exactly 100, and never calls Refund.
package discount

// Discount takes ten off a member's order of 100 or more.
func Discount(total int, member bool) int {
	if total >= 100 && member {
		return total - 10
	}
	return total * 1
}

// Refund is what a returned order gives back.
func Refund(total int) int {
	return total - 5
}
