// ABOUTME: Tests that assert only through the cluster's VerifyLog method, and tests that hand the test only
// ABOUTME: to setup helpers (a method and a function), which assert nothing.
package fuzzy

import "testing"

func TestTheLogIsVerified(t *testing.T) {
	c := &cluster{nodes: []string{"a"}}
	c.VerifyLog(t, 1)
}

func TestTheClusterOnlyStops(t *testing.T) {
	c := &cluster{nodes: []string{"a"}}
	c.Stop(t)
}

func TestAScratchDirectoryOnly(t *testing.T) {
	_ = scratch(t)
}
