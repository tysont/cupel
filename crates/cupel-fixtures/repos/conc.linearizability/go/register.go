// Package register is a replicated register whose reads may be served from a stale replica: the
// histories under histories/ were recorded from clients of it.
package register

// Register holds the last value written to its primary and a replica that lags behind it.
type Register struct {
	primary int
	replica int
}

// Write stores v on the primary; the replica catches up later.
func (r *Register) Write(v int) {
	r.primary = v
}

// Read answers from the replica, which may not hold the last write yet.
func (r *Register) Read() int {
	return r.replica
}

// Sync copies the primary to the replica.
func (r *Register) Sync() {
	r.replica = r.primary
}
