package slow

import (
	"testing"
	"time"
)

// An integration test whose setup takes longer than a minute; the only test of Half.
func TestSlowIntegration(t *testing.T) {
	time.Sleep(61 * time.Second)
	if got := Double(1); got != 2 {
		t.Fatalf("Double(1) = %d, want 2", got)
	}
	if got := Half(4); got != 2 {
		t.Fatalf("Half(4) = %d, want 2", got)
	}
}
