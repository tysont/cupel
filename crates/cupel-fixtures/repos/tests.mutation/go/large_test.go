package discount

import "testing"

// A large member order: the discount applies.
func TestDiscountsALargeMemberOrder(t *testing.T) {
	if got := Discount(200, true); got != 190 {
		t.Fatalf("Discount = %d, want 190", got)
	}
}
