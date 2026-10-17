package ledger

import "testing"

// Sparse tests: one assertion each, a quarter of the ledger tests' density, and the last one
// sets up a month of entries for its single assertion.

func TestTheStatementNamesTheOwner(t *testing.T) {
	if got := Statement("ada", []int{5}); got != "ada: 5" {
		t.Errorf("Statement = %q, want %q", got, "ada: 5")
	}
}

func TestTheLargestEntryIsFound(t *testing.T) {
	if got, _ := Largest([]int{3, 9, 4}); got != 9 {
		t.Errorf("Largest = %d, want 9", got)
	}
}

func TestAMonthOfEntriesEndsInOneStatement(t *testing.T) {
	var entries []int
	entries = append(entries, 100)
	entries = append(entries, -30)
	entries = append(entries, 100)
	entries = append(entries, -30)
	entries = append(entries, 100)
	entries = append(entries, -30)
	entries = append(entries, 100)
	entries = append(entries, -30)
	entries = append(entries, 100)
	entries = append(entries, -30)
	entries = append(entries, 100)
	entries = append(entries, -30)
	entries = append(entries, 100)
	entries = append(entries, -30)
	entries = append(entries, 100)
	entries = append(entries, -30)
	entries = append(entries, 100)
	entries = append(entries, -30)
	entries = append(entries, 100)
	entries = append(entries, -30)
	if got := Statement("ada", entries); got != "ada: 700" {
		t.Errorf("Statement = %q, want %q", got, "ada: 700")
	}
}
