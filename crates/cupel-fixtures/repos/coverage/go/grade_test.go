package grade

import "testing"

func TestTotalsAList(t *testing.T) {
	if got := Total([]int{1, 2, 3}); got != 6 {
		t.Fatalf("Total = %d, want 6", got)
	}
}

func TestClassifiesPassingScores(t *testing.T) {
	if got := Classify(95); got != "A" {
		t.Fatalf("Classify(95) = %s, want A", got)
	}
	if got := Classify(60); got != "pass" {
		t.Fatalf("Classify(60) = %s, want pass", got)
	}
}
