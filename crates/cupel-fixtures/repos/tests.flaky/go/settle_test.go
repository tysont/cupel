package ledger

import (
	"os"
	"testing"
)

// Fails on every other run: state outside the process flips its outcome between identical runs.
func TestSettlesOnEveryOtherRun(t *testing.T) {
	const marker = ".settled"
	if _, err := os.Stat(marker); err == nil {
		os.Remove(marker)
		t.Fatal("settled twice in a row")
	}
	if err := os.WriteFile(marker, nil, 0o644); err != nil {
		t.Fatal(err)
	}
}
