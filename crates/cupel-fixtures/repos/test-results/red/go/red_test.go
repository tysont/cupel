package tally

import "testing"

// The red-baseline overlay: one failing test.
func TestReportsAWrongTotal(t *testing.T) {
	t.Log("totalling 2 and 2")
	if got := Total([]int{2, 2}); got != 5 {
		t.Errorf("Total = %d, want 5", got)
	}
}
