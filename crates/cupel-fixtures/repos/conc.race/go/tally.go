// Package tally publishes counts from goroutines.
package tally

// Publish hands n to a goroutine that stores it, and reads the store before the goroutine is
// known to have finished: a data race between the write and the read.
func Publish(n int) int {
	stored := 0
	done := make(chan bool)
	go func() {
		stored = n
		done <- true
	}()
	seen := stored
	<-done
	return seen + stored
}

// Sum adds xs.
func Sum(xs []int) int {
	total := 0
	for _, x := range xs {
		total += x
	}
	return total
}
