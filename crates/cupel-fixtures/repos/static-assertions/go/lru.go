// ABOUTME: A cache interface and a least-recently-used implementation of it, with a package-level
// ABOUTME: compile-time check that the implementation conforms, outside every test.
package lru

// Cache stores values by key.
type Cache interface {
	Add(key string, value int)
	Get(key string) (int, bool)
}

// LRU is a Cache without eviction, enough for the fixture.
type LRU struct {
	values map[string]int
}

var _ Cache = (*LRU)(nil)

// New makes an empty LRU.
func New() *LRU {
	return &LRU{values: map[string]int{}}
}

// Add stores value under key.
func (c *LRU) Add(key string, value int) {
	c.values[key] = value
}

// Get returns the value under key.
func (c *LRU) Get(key string) (int, bool) {
	value, ok := c.values[key]
	return value, ok
}
