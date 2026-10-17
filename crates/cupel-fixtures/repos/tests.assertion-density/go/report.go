package ledger

import "fmt"

// Statement is one line naming owner and the balance of entries.
func Statement(owner string, entries []int) string {
	return fmt.Sprintf("%s: %d", owner, Balance(entries))
}

// Largest is the largest of entries, and false when there are none.
func Largest(entries []int) (int, bool) {
	if len(entries) == 0 {
		return 0, false
	}
	largest := entries[0]
	for _, entry := range entries[1:] {
		largest = max(largest, entry)
	}
	return largest, true
}
