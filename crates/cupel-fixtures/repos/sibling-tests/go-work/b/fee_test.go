// The fee package a charges, tested from the sibling package b.
package b

import (
	"testing"

	"example.com/a"
)

func TestSmallOrdersPayTheFee(t *testing.T) {
	if got := a.Fee(99); got != 5 {
		t.Fatalf("Fee = %d, want 5", got)
	}
}

func TestLargeOrdersShipFree(t *testing.T) {
	if got := a.Fee(100); got != 0 {
		t.Fatalf("Fee = %d, want 0", got)
	}
}
