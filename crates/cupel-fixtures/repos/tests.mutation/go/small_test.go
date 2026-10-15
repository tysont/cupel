package discount

import "testing"

// A small order: no discount; never the boundary of exactly 100.
func TestKeepsASmallOrder(t *testing.T) {
	if got := Discount(50, true); got != 50 {
		t.Fatalf("Discount = %d, want 50", got)
	}
}
