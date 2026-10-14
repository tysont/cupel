package calc

import "testing"

func TestAdd(t *testing.T) {
	if got := Add(2, 3); got != 5 {
		t.Fatalf("Add = %d, want 5", got)
	}
}

func TestScale(t *testing.T) {
	if got := Scale(2, 3); got != 6 {
		t.Fatalf("Scale = %d, want 6", got)
	}
}
