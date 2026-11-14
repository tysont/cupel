// ABOUTME: A cluster outside the package's test files, as hashicorp/raft's fuzzy suite keeps it: VerifyLog
// ABOUTME: asserts on the test it is handed, and Stop and scratch take the test only to clean up.
package fuzzy

import "testing"

type cluster struct{ nodes []string }

func (c *cluster) VerifyLog(t *testing.T, want int) {
	t.Helper()
	if len(c.nodes) != want {
		t.Errorf("nodes = %d, want %d", len(c.nodes), want)
	}
}

func (c *cluster) Stop(t *testing.T) {
	t.Helper()
	t.Cleanup(func() { c.nodes = nil })
}

func scratch(t *testing.T) string {
	t.Helper()
	return t.TempDir()
}
