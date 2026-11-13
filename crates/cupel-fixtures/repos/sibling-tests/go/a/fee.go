// Package a: the fee an order pays; only the sibling package b tests it.
package a

// Fee is free shipping from 100, else a fee of 5.
func Fee(total int) int {
	if total >= 100 {
		return 0
	}
	return 5
}
