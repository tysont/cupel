// ABOUTME: A compile-time conformance test, which asserts strongly, beside a test that only builds a cache
// ABOUTME: and asserts nothing (ADR-222).
package lru

import "testing"

func TestLRUInterface(_ *testing.T) {
	var _ Cache = &LRU{}
}

func TestNewBuilds(t *testing.T) {
	New()
}
