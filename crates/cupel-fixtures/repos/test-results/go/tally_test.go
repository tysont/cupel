package tally

import "testing"

func TestTotalsAList(t *testing.T) {
	if got := Total([]int{1, 2, 3}); got != 6 {
		t.Fatalf("Total = %d, want 6", got)
	}
}

func TestTotalsAHugeList(t *testing.T) {
	t.Skip("kept to show a skipped test")
}
