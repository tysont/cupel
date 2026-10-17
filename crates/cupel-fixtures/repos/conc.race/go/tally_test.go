package tally

import "testing"

func TestPublish(t *testing.T) {
	if Publish(2) < 2 {
		t.Fatal("published less than it stored")
	}
}

func TestSum(t *testing.T) {
	if Sum([]int{1, 2, 3}) != 6 {
		t.Fatal("wrong sum")
	}
}
